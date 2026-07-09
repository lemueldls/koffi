pub mod c;
pub mod filters;
pub mod kotlin;
pub mod rust;

use askama::Template;
use koffi_ir::{CrateInterface, FFIType, ReceiverType};

use crate::codegen::GlueDependency;

#[derive(Template)]
#[template(path = "kotlin/common.kt.j2", escape = "none")]
pub struct KotlinCommonTemplate<'a> {
    pub crate_name: &'a str,
    pub ir: &'a CrateInterface,
}

#[derive(Template)]
#[template(path = "kotlin/jvm.kt.j2", escape = "none")]
pub struct KotlinJvmTemplate<'a> {
    pub pkg_pascal: &'a str, // PascalCase crate name, used for JNI object name
    pub crate_name: &'a str, // snake_case library name for System.loadLibrary
    pub lib_name: &'a str,   // actual .so/.dylib name without prefix/ext
    pub ir: &'a CrateInterface,
}

#[derive(Template)]
#[template(path = "kotlin/native.kt.j2", escape = "none")]
pub struct KotlinNativeTemplate<'a> {
    pub crate_ident: &'a str, // crate name with hyphens -> underscores
    pub namespace: &'a str,
    pub ir: &'a CrateInterface,
}

#[derive(Template)]
#[template(path = "kotlin/wasm.kt.j2", escape = "none")]
pub struct KotlinWasmTemplate<'a> {
    pub crate_ident: &'a str,
    pub namespace: &'a str,
    pub lib_name: &'a str,
    pub emit_handle_release: bool,
    pub ir: &'a CrateInterface,
}

#[derive(Template)]
#[template(path = "kotlin/loader.kt.j2", escape = "none")]
pub struct KotlinLoaderTemplate<'a> {
    pub namespace: &'a str,
    pub pkg_pascal: &'a str,
    pub lib_name: &'a str,
}

#[derive(Template)]
#[template(path = "kotlin/module.yaml.j2", escape = "none")]
pub struct KotlinModuleTemplate {
    pub platforms: Vec<&'static str>,
}

#[derive(Template)]
#[template(path = "rust/jni_glue.rs.j2", escape = "none")]
pub struct RustJniTemplate<'a> {
    pub pkg_pascal: &'a str,
    pub crate_ident: &'a str, // crate name with hyphens -> underscores
    pub ir: &'a CrateInterface,
    pub emit_handle_release: bool,
}

#[derive(Template)]
#[template(path = "rust/cabi_glue.rs.j2", escape = "none")]
pub struct RustCabiTemplate<'a> {
    pub crate_ident: &'a str,
    pub ir: &'a CrateInterface,
}

#[derive(Template)]
#[template(path = "rust/wasm_glue.rs.j2", escape = "none")]
pub struct RustWasmTemplate<'a> {
    pub crate_ident: &'a str,
    pub ir: &'a CrateInterface,
    pub emit_handle_release: bool,
}

#[derive(Template)]
#[template(path = "native/header.h.j2", escape = "none")]
pub struct CHeaderTemplate<'a> {
    pub crate_ident: &'a str,
    pub guard: &'a str, // UPPER_SNAKE_CASE header guard
    pub ir: &'a CrateInterface,
}

#[derive(Template)]
#[template(path = "native/cinterop.def.j2", escape = "none")]
pub struct CinteropDefTemplate<'a> {
    pub crate_ident: &'a str,
    pub namespace: &'a str,
    pub lib_name: &'a str,
    pub include_dir: &'a str,
}

#[derive(Template)]
#[template(path = "rust/lib.rs.j2", escape = "none")]
pub struct GlueLibTemplate {
    pub cabi_modules: Vec<String>,
    pub jni_modules: Vec<String>,
    pub wasm_modules: Vec<String>,
}

#[derive(Template)]
#[template(path = "rust/cargo.toml.j2", escape = "none")]
pub struct GlueCargoTemplate<'a> {
    pub crate_name: &'a str,
    pub version: &'a str,
    pub dependencies: Vec<GlueDependency>,
}
