use std::{fs, path::Path};

use askama::Template;

mod c;
mod kotlin;
mod rust;
mod template;

use koffi_build::OutputDirs;
use pathdiff::diff_paths;
pub use template::*;

use crate::schema::Schema;

pub fn render_all(schema: &Schema, dirs: &OutputDirs) -> anyhow::Result<()> {
    render_rust(schema, &dirs.crate_path, &dirs.rust_out_dir)?;
    render_kotlin(schema, &dirs.kotlin_out_dir)?;

    Ok(())
}

fn render_rust(schema: &Schema, crate_path: &Path, rust_out_dir: &Path) -> anyhow::Result<()> {
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
            required_dependencies: &[
                CargoDep {
                    name: schema.crate_name.clone(),
                    path: rel_crate_path,
                },
                CargoDep {
                    name: "koffi".to_string(),
                    path: "../../../../crates/api".to_string(),
                },
            ],
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
        },
        &rust_out_dir.join("Cargo.toml"),
    )?;

    Ok(())
}

fn render_kotlin(schema: &Schema, kotlin_out_dir: &Path) -> anyhow::Result<()> {
    let kotlin_ident = schema.crate_ident_pascal();

    write_template(
        KotlinCommon { schema },
        &kotlin_out_dir
            .join("src")
            .join(format!("{kotlin_ident}.kt")),
    )?;
    write_template(
        KotlinFfm { schema },
        &kotlin_out_dir
            .join("src@jvm")
            .join(format!("{kotlin_ident}.kt")),
    )?;
    write_template(
        KotlinModuleYaml {
            platforms: &["jvm"],
            dependencies: &["../../kotlin/runtime"],
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
