use askama::Template;

use crate::{
    layout::LayoutEntry,
    schema::{Schema, SchemaTypeRef},
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
#[template(path = "kotlin/module.yaml.j2")]
pub struct KotlinModuleYaml<'a> {
    pub platforms: &'a [&'a str],
    pub dependencies: &'a [&'a str],
}
