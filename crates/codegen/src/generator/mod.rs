use std::{fs, path::Path};

use anyhow::bail;
use askama::Template;

mod c;
mod kotlin;
mod rust;
mod template;

use koffi_build::{OutputDirs, config::TargetPlatform};
use pathdiff::diff_paths;
pub use template::*;

use crate::{config::KoffiConfig, schema::Schema};

pub fn render_all(schema: &Schema, dirs: &OutputDirs, config: &KoffiConfig) -> anyhow::Result<()> {
    render_rust(schema, &dirs.crate_path, &dirs.rust_out_dir, config)?;
    render_kotlin(schema, &dirs.kotlin_out_dir, config)?;

    Ok(())
}

fn render_rust(
    schema: &Schema,
    crate_path: &Path,
    rust_out_dir: &Path,
    config: &KoffiConfig,
) -> anyhow::Result<()> {
    let src_dir = rust_out_dir.join("src");
    write_template(RustLib { schema }, &src_dir.join("lib.rs"))?;
    write_template(RustTypes { schema }, &src_dir.join("types.rs"))?;
    write_template(RustCabi { schema }, &src_dir.join("cabi.rs"))?;
    write_template(RustJni { schema }, &src_dir.join("jni.rs"))?;
    write_template(RustWasm { schema }, &src_dir.join("wasm.rs"))?;

    let rel_crate_path = diff_paths(crate_path, rust_out_dir)
        .expect("should be able to relativize crate path")
        .display()
        .to_string()
        .replace('\\', "/");

    write_template(
        RustCargoToml {
            crate_name: &schema.glue_crate_ident(),
            required_dependencies: &[CargoDep {
                name: schema.crate_name.clone(),
                path: rel_crate_path,
            }],
            optional_dependencies: &[
                CargoOptionalDep {
                    name: "jni".to_string(),
                    version: "0.22".to_string(),
                    feature: "jni".to_string(),
                },
                CargoOptionalDep {
                    name: "wasm-bindgen".to_string(),
                    version: "0.2".to_string(),
                    feature: "wasm".to_string(),
                },
            ],
            // cinterop links the glue's staticlib: only a native platform
            // needs the `.a` in the build output.
            crate_types: if config.native_platforms().is_empty() {
                &["cdylib"]
            } else {
                &["cdylib", "staticlib"]
            },
        },
        &rust_out_dir.join("Cargo.toml"),
    )?;

    Ok(())
}

fn render_kotlin(
    schema: &Schema,
    kotlin_out_dir: &Path,
    config: &KoffiConfig,
) -> anyhow::Result<()> {
    let kotlin_ident = schema.crate_ident_pascal();

    write_template(
        KotlinCommon { schema },
        &kotlin_out_dir
            .join("src")
            .join(format!("{kotlin_ident}.kt")),
    )?;

    if config.has(&TargetPlatform::Jvm) {
        if config.jvm_uses_jni() {
            write_template(
                KotlinJni {
                    schema,
                    android: false,
                },
                &kotlin_out_dir
                    .join("src@jvm")
                    .join(format!("{kotlin_ident}.kt")),
            )?;
        } else {
            write_template(
                KotlinFfm { schema },
                &kotlin_out_dir
                    .join("src@jvm")
                    .join(format!("{kotlin_ident}.kt")),
            )?;
        }
    }

    if config.has(&TargetPlatform::Android) {
        write_template(
            KotlinJni {
                schema,
                android: true,
            },
            &kotlin_out_dir
                .join("src@android")
                .join(format!("{kotlin_ident}.kt")),
        )?;
    }

    let native_platforms = config.native_platforms();
    if !native_platforms.is_empty() {
        for s in &schema.structs {
            if s.layout.total.size == 0 {
                bail!(
                    "koffi M0: zero-size struct `{}` can't be represented in a C header; \
                     drop the native platforms from platforms or change the struct",
                    s.name
                );
            }
        }
        write_template(
            KotlinNative { schema },
            &kotlin_out_dir
                .join("src@native")
                .join(format!("{kotlin_ident}.kt")),
        )?;
        write_template(
            CHeader { schema },
            &kotlin_out_dir
                .join("cinterop")
                .join(format!("{}.h", schema.crate_ident())),
        )?;
        write_template(
            CInteropDef {
                schema,
                native_platforms: &native_platforms,
            },
            &kotlin_out_dir
                .join("cinterop")
                .join(format!("{}.def", schema.crate_ident())),
        )?;
    }

    write_template(
        KotlinModuleYaml {
            platforms: &config.platforms,
            dependencies: &[],
            android: config.has(&TargetPlatform::Android),
        },
        &kotlin_out_dir.join("module.yaml"),
    )?;

    Ok(())
}

fn write_template(template: impl Template, path: &Path) -> anyhow::Result<()> {
    let rendered = template.render()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, rendered)?;

    Ok(())
}
