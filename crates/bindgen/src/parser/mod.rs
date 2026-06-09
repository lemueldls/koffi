pub mod context;
pub mod rustdoc;
pub mod visitor;

use std::path::Path;

use koffi_ir::{CrateInterface, EnumInfo, FnInfo, StructInfo};

use crate::{BindgenError, meta::KoffiPackageMeta};

#[derive(Debug)]
pub struct PartialInterface {
    pub namespace: String,
    pub crate_name: String,
    pub version: String,
    /// Types are named but not yet fully resolved.
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
    pub functions: Vec<FnInfo>,
}

/// Full parse pipeline: syn pass -> rustdoc resolution -> [`CrateInterface`].
pub fn parse_crate(
    crate_path: &Path,
    workspace_root: &Path,
    crate_name: String,
    version: String,
    meta: &KoffiPackageMeta,
    dep_schemas: &[CrateInterface], // pre-loaded from dependency schema.json files
) -> Result<CrateInterface, BindgenError> {
    let crate_namespace = meta
        .namespace
        .clone()
        .unwrap_or_else(|| "generated".to_string());

    let partial = visitor::parse_syn(crate_path, crate_namespace, crate_name, version)?;

    let rustdoc_path = rustdoc::ensure_json(crate_path, workspace_root)?;
    let resolved = rustdoc::resolve_types(partial, &rustdoc_path, dep_schemas)?;

    Ok(resolved)
}
