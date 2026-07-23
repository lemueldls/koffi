pub mod templates;

use std::{
    fs,
    path::{Path, PathBuf},
};

use askama::Template;
use heck::AsPascalCase;
use koffi_ir::CrateInterface;
use pathdiff::diff_paths;

use crate::{BindgenError, build_steps::TargetPlatforms};

#[derive(Debug)]
pub struct GeneratedPaths {
    pub kotlin_common: PathBuf,
    pub kotlin_jvm: PathBuf,
    pub kotlin_native: PathBuf,
    pub kotlin_wasm: PathBuf,
    pub kotlin_loader: PathBuf,
    pub kotlin_module: PathBuf,
    pub rust_jni_glue: PathBuf,
    pub rust_cabi_glue: PathBuf,
    pub rust_wasm_glue: PathBuf,
    pub c_header: PathBuf,
    pub cinterop_def: PathBuf,
    pub glue_lib: PathBuf,
    pub glue_cargo_toml: PathBuf,
}

pub struct BindingPackage<'a> {
    pub ir: &'a CrateInterface,
    pub crate_path: &'a Path,
    pub is_root: bool,
}

#[derive(Debug)]
pub struct GlueDependency {
    pub package_name: String,
    pub path: String,
}

pub fn generate_package_set(
    packages: &[BindingPackage<'_>],
    glue_crate_name: &str,
    version: &str,
    out_dir: &Path,
    target_platforms: &TargetPlatforms,
) -> Result<(), BindgenError> {
    let glue_crate_ident = glue_crate_name.replace('-', "_");
    let lib_name = format!("{glue_crate_ident}_koffi_glue");

    let kotlin_dir = out_dir.join("kotlin");
    let jni_dir = kotlin_dir.join("jniLibs");
    let res_dir = kotlin_dir.join("resources@jvm/native");
    let include_dir = kotlin_dir.join("include");
    let cinterop_dir = kotlin_dir.join("cinterop");

    let rust_dir = out_dir.join("rust");
    let rust_src = rust_dir.join("src");

    for dir in [
        &kotlin_dir,
        &jni_dir,
        &res_dir,
        &include_dir,
        &cinterop_dir,
        &rust_src,
    ] {
        fs::create_dir_all(dir)?;
    }

    write_template(
        &templates::KotlinModuleTemplate {
            platforms: target_platforms.platforms(),
        },
        &kotlin_dir.join("module.yaml"),
    )?;

    let mut cabi_modules = Vec::new();
    let mut jni_modules = Vec::new();
    let mut wasm_modules = Vec::new();
    let mut dependencies = Vec::new();

    for package in packages {
        let ir = package.ir;
        let is_root = package.is_root;
        let crate_name = &ir.crate_name;
        let crate_ident = crate_name.replace('-', "_");
        let pkg_pascal = AsPascalCase(crate_name).to_string();
        let guard = format!("{}_H", crate_ident.to_uppercase());
        let pkg_file = format!("{pkg_pascal}.kt");
        let namespace = &ir.namespace;

        write_template(
            &templates::KotlinCommonTemplate { crate_name, ir },
            &kotlin_dir
                .join("src")
                .tap(|d| {
                    let _ = fs::create_dir_all(d);
                })
                .join(&pkg_file),
        )?;

        write_template(
            &templates::KotlinJvmTemplate {
                pkg_pascal: &pkg_pascal,
                crate_name,
                lib_name: &lib_name,
                ir,
            },
            &kotlin_dir
                .join("src@jvm")
                .tap(|d| {
                    let _ = fs::create_dir_all(d);
                })
                .join(&pkg_file),
        )?;

        write_template(
            &templates::KotlinLoaderTemplate {
                namespace,
                pkg_pascal: &pkg_pascal,
                lib_name: &lib_name,
            },
            &kotlin_dir
                .join("src@jvm")
                .join(format!("{pkg_pascal}Loader.kt")),
        )?;

        write_template(
            &templates::KotlinNativeTemplate {
                crate_ident: &crate_ident,
                namespace,
                ir,
            },
            &kotlin_dir
                .join("src@native")
                .tap(|d| {
                    let _ = fs::create_dir_all(d);
                })
                .join(&pkg_file),
        )?;

        write_template(
            &templates::KotlinWasmTemplate {
                crate_ident: &crate_ident,
                namespace,
                ir,
                lib_name: &lib_name,
                is_root,
            },
            &kotlin_dir
                .join("src@web")
                .tap(|d| {
                    let _ = fs::create_dir_all(d);
                })
                .join(&pkg_file),
        )?;

        let jni_module = format!("{crate_ident}_jni_glue");
        write_template(
            &templates::RustJniTemplate {
                pkg_pascal: &pkg_pascal,
                crate_ident: &crate_ident,
                ir,
                is_root,
            },
            &rust_src.join(format!("{jni_module}.rs")),
        )?;
        jni_modules.push(jni_module);

        let cabi_module = format!("{crate_ident}_cabi_glue");
        write_template(
            &templates::RustCabiTemplate {
                crate_ident: &crate_ident,
                ir,
            },
            &rust_src.join(format!("{cabi_module}.rs")),
        )?;
        cabi_modules.push(cabi_module);

        let wasm_module = format!("{crate_ident}_wasm_glue");
        write_template(
            &templates::RustWasmTemplate {
                crate_ident: &crate_ident,
                ir,
                is_root,
            },
            &rust_src.join(format!("{wasm_module}.rs")),
        )?;
        wasm_modules.push(wasm_module);

        if package.is_root {
            write_template(
                &templates::CHeaderTemplate {
                    crate_ident: &crate_ident,
                    guard: &guard,
                    ir,
                },
                &include_dir.join(format!("{crate_ident}.h")),
            )?;

            write_template(
                &templates::CinteropDefTemplate {
                    crate_ident: &crate_ident,
                    namespace,
                    lib_name: &lib_name,
                    include_dir: &include_dir.display().to_string().replace('\\', "/"),
                },
                &cinterop_dir.join(format!("{crate_ident}.def")),
            )?;
        }

        let rel_crate_path = diff_paths(package.crate_path, &rust_dir)
            .expect("should be able to relativize crate path")
            .display()
            .to_string()
            .replace('\\', "/");

        dependencies.push(GlueDependency {
            package_name: crate_name.clone(),
            path: rel_crate_path,
        });

        copy_kotlin_source(&package.crate_path.join("kotlin"), &kotlin_dir)?;
    }

    write_template(
        &templates::GlueLibTemplate {
            cabi_modules,
            jni_modules,
            wasm_modules,
        },
        &rust_src.join("lib.rs"),
    )?;

    write_template(
        &templates::GlueCargoTemplate {
            crate_name: glue_crate_name,
            version,
            dependencies,
        },
        &rust_dir.join("Cargo.toml"),
    )?;

    fs::write(&out_dir.join(".gitignore"), "*\n")?; // ignore all files in the generated directory

    Ok(())
}

fn write_template<T: Template>(t: &T, path: &Path) -> Result<PathBuf, BindgenError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let rendered = t.render()?;
    fs::write(path, rendered)?;

    Ok(path.to_path_buf())
}

pub fn copy_kotlin_source(source_dir: &Path, kotlin_dir: &Path) -> Result<(), BindgenError> {
    let common_src = source_dir.join("src");
    let common_dst = kotlin_dir.join("src");
    copy_kotlin_prebuilt(&common_src, &common_dst)?;

    let platforms = ["android", "jvm", "native", "web"];
    for platform in platforms {
        let src = source_dir.join(format!("src@{platform}"));
        let dst = kotlin_dir.join(format!("src@{platform}"));
        copy_kotlin_prebuilt(&src, &dst)?;
    }

    Ok(())
}

fn copy_kotlin_prebuilt(src: &Path, kotlin_dir: &Path) -> Result<(), BindgenError> {
    if matches!(fs::exists(src), Ok(true)) {
        fs::create_dir_all(kotlin_dir)?;
        for entry in walkdir::WalkDir::new(src) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let rel = entry.path().strip_prefix(src)?;
                let dst = kotlin_dir.join(rel);
                if let Some(p) = dst.parent() {
                    fs::create_dir_all(p)?;
                }
                fs::copy(entry.path(), dst)?;
            }
        }
    }

    Ok(())
}

pub fn emit_schema(ir: &CrateInterface, path: &Path) -> Result<(), BindgenError> {
    let json = facet_json::to_string_pretty(ir)?;
    fs::write(path, json)?;

    Ok(())
}

// Minimal tap() for ergonomic dir-creation inline with path construction
trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&Self)) -> Self {
        f(&self);
        self
    }
}

impl<T> Tap for T {}
