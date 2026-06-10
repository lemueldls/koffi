pub mod templates;

use std::{
    fs,
    path::{Path, PathBuf},
};

use askama::Template;
use heck::AsPascalCase;
use koffi_ir::CrateInterface;
use pathdiff::diff_paths;

use crate::BindgenError;

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

pub fn generate_all(
    ir: &CrateInterface,
    out_dir: &Path,
    crate_path: &Path,
    runtime_path: &Path,
) -> Result<GeneratedPaths, BindgenError> {
    let crate_name = &ir.crate_name;
    let crate_ident = crate_name.replace('-', "_");
    let pkg_pascal = AsPascalCase(crate_name).to_string();
    let lib_name = format!("{crate_ident}_koffi_glue");
    let guard = format!("{}_H", crate_ident.to_uppercase());

    let kotlin_dir = out_dir.join("kotlin");
    let jni_dir = kotlin_dir.join("jniLibs");
    let res_dir = kotlin_dir.join("resources@jvm/natives");
    let cinterop = kotlin_dir.join("cinterop");

    let rust_dir = out_dir.join("rust");
    let rust_src = rust_dir.join("src");

    for dir in [&kotlin_dir, &jni_dir, &res_dir, &cinterop, &rust_src] {
        fs::create_dir_all(dir)?;
    }

    let pkg_file = format!("{pkg_pascal}.kt");

    let common_path = write_template(
        &templates::KotlinCommonTemplate {
            namespace: &ir.namespace,
            crate_name,
            ir,
        },
        &kotlin_dir
            .join("src")
            .tap(|d| {
                let _ = fs::create_dir_all(d);
            })
            .join(&pkg_file),
    )?;

    let jvm_path = write_template(
        &templates::KotlinJvmTemplate {
            namespace: &ir.namespace,
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

    let loader_path = write_template(
        &templates::KotlinLoaderTemplate {
            namespace: &ir.namespace,
            pkg_pascal: &pkg_pascal,
            lib_name: &lib_name,
        },
        &kotlin_dir
            .join("src@jvm")
            .join(format!("{pkg_pascal}Loader.kt")),
    )?;

    let module_path = write_template(
        &templates::KotlinModuleTemplate {
            namespace: &ir.namespace,
            pkg_pascal: &pkg_pascal,
            crate_name,
        },
        &kotlin_dir.join("module.yaml"),
    )?;

    let native_path = write_template(
        &templates::KotlinNativeTemplate {
            crate_ident: &crate_ident,
            namespace: &ir.namespace,
            cinterop_pkg: &format!("{crate_ident}.cinterop"),
            ir,
        },
        &kotlin_dir
            .join("src@native")
            .tap(|d| {
                let _ = fs::create_dir_all(d);
            })
            .join(&pkg_file),
    )?;

    let wasm_path = write_template(
        &templates::KotlinWasmTemplate {
            crate_ident: &crate_ident,
            namespace: &ir.namespace,
            ir,
        },
        &kotlin_dir
            .join("src@web")
            .tap(|d| {
                let _ = fs::create_dir_all(d);
            })
            .join(&pkg_file),
    )?;

    let jni_path = write_template(
        &templates::RustJniTemplate {
            namespace: &ir.namespace,
            pkg_pascal: &pkg_pascal,
            crate_ident: &crate_ident,
            ir,
        },
        &rust_src.join("jni_glue.rs"),
    )?;

    let cabi_path = write_template(
        &templates::RustCabiTemplate {
            crate_ident: &crate_ident,
            ir,
        },
        &rust_src.join("cabi_glue.rs"),
    )?;

    let web_path = write_template(
        &templates::RustWasmTemplate {
            crate_ident: &crate_ident,
            ir,
        },
        &rust_src.join("wasm_glue.rs"),
    )?;

    let header_path = write_template(
        &templates::CHeaderTemplate {
            crate_name,
            guard: &guard,
            ir,
        },
        &cinterop.join(format!("{crate_name}.h")),
    )?;

    let def_path = write_template(
        &templates::CinteropDefTemplate {
            crate_name,
            lib_name: &lib_name,
        },
        &cinterop.join(format!("{crate_name}.def")),
    )?;

    let lib_path = write_template(
        &templates::GlueLibTemplate {
            crate_ident: &crate_ident,
            ir,
        },
        &rust_src.join("lib.rs"),
    )?;

    let rel_crate_path = diff_paths(crate_path, &rust_dir)
        .expect("should be able to relativize crate path")
        .display()
        .to_string()
        .replace('\\', "/");
    let rel_runtime_path = diff_paths(runtime_path, &rust_dir)
        .expect("should be able to relativize runtime path")
        .display()
        .to_string()
        .replace('\\', "/");
    let cargo_path = write_template(
        &templates::GlueCargoTemplate {
            crate_name,
            version: &ir.version,
            crate_path: &rel_crate_path,
            runtime_path: &rel_runtime_path,
        },
        &rust_dir.join("Cargo.toml"),
    )?;

    Ok(GeneratedPaths {
        kotlin_common: common_path,
        kotlin_jvm: jvm_path,
        kotlin_native: native_path,
        kotlin_wasm: web_path,
        kotlin_loader: loader_path,
        kotlin_module: module_path,
        rust_jni_glue: jni_path,
        rust_cabi_glue: cabi_path,
        rust_wasm_glue: wasm_path,
        c_header: header_path,
        cinterop_def: def_path,
        glue_lib: lib_path,
        glue_cargo_toml: cargo_path,
    })
}

pub fn copy_runtime(runtime_path: &Path, out_dir: &Path) -> Result<(), BindgenError> {
    let common_src = runtime_path.join("src");
    let common_dst = out_dir.join("kotlin").join("src");
    fs::create_dir_all(&common_dst)?;
    for entry in fs::read_dir(common_src)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            fs::copy(entry.path(), common_dst.join(entry.file_name()))?;
        }
    }

    let platforms = ["android", "jvm", "native", "web"];
    for platform in platforms {
        let src = runtime_path.join(format!("src@{platform}"));
        let dst = out_dir.join("kotlin").join(format!("src@{platform}"));
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::copy(entry.path(), dst.join(entry.file_name()))?;
            }
        }
    }

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

// Minimal tap() for ergonomic dir-creation inline with path construction
trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&Self)) -> Self {
        f(&self);
        self
    }
}

impl<T> Tap for T {}
