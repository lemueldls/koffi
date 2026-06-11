pub mod context;
pub mod rustdoc;
pub mod visitor;

use std::path::Path;

use koffi_ir::CrateInterface;

use crate::{BindgenError, meta::KoffiPackageMeta};

/// Full parse pipeline: syn pass -> rustdoc resolution -> [`CrateInterface`].
pub fn parse_crate(
    crate_path: &Path,
    _workspace_root: &Path,
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

    let rustdoc_path = rustdoc::ensure_json(crate_path)?;
    let resolved = rustdoc::resolve_types(partial, &rustdoc_path, dep_schemas)?;

    Ok(resolved)
}

pub fn parse_crate_syn_only(
    crate_path: &Path,
    crate_name: String,
    version: String,
    meta: &KoffiPackageMeta,
) -> Result<CrateInterface, BindgenError> {
    let crate_namespace = meta
        .namespace
        .clone()
        .unwrap_or_else(|| "generated".to_string());

    let ir = visitor::parse_syn(crate_path, crate_namespace, crate_name, version)?;

    Ok(CrateInterface {
        namespace: ir.namespace,
        crate_name: ir.crate_name,
        version: ir.version,
        structs: ir.structs,
        enums: ir.enums,
        functions: ir.functions,
        imports: vec![],
    })
}
