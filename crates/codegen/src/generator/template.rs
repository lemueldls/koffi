use askama::Template;
use koffi_build::config::TargetPlatform;

use crate::{
    layout::LayoutEntry,
    schema::{ScalarKind, Schema, SchemaTypeRef},
};

#[derive(Template)]
#[template(path = "rust/lib.rs.j2")]
pub struct RustLib<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "rust/types.rs.j2")]
pub struct RustTypes<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "rust/cabi.rs.j2")]
pub struct RustCabi<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "rust/jni.rs.j2")]
pub struct RustJni<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "rust/wasm.rs.j2")]
pub struct RustWasm<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "rust/cargo.toml.j2")]
pub struct RustCargoToml<'a> {
    pub crate_name: &'a str,
    pub required_dependencies: &'a [CargoDep],
    pub optional_dependencies: &'a [CargoOptionalDep],
    pub crate_types: &'a [&'a str],
}

pub struct CargoDep {
    pub name: String,
    pub path: String,
}

pub struct CargoOptionalDep {
    pub name: String,
    pub version: String,
    pub feature: String,
}

#[derive(Template)]
#[template(path = "kotlin/common.kt.j2")]
pub struct KotlinCommon<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "kotlin/ffm.kt.j2")]
pub struct KotlinFfm<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "kotlin/jni.kt.j2")]
pub struct KotlinJni<'a> {
    pub schema: &'a Schema,
    /// Android load mode (`System.loadLibrary` + jniLibs staging) vs. the
    /// jvm classpath-resource extraction loader.
    pub android: bool,
}

#[derive(Template)]
#[template(path = "kotlin/native.kt.j2")]
pub struct KotlinNative<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "kotlin/module.yaml.j2")]
pub struct KotlinModuleYaml<'a> {
    pub platforms: &'a [TargetPlatform],
    pub dependencies: &'a [&'a str],
    pub android: bool,
}

#[derive(Template)]
#[template(path = "native/header.c.j2")]
pub struct CHeader<'a> {
    pub schema: &'a Schema,
}

#[derive(Template)]
#[template(path = "native/cinterop.def.j2")]
pub struct CInteropDef<'a> {
    pub schema: &'a Schema,
    pub native_platforms: &'a [&'a TargetPlatform],
}
