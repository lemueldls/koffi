pub mod context;
pub mod rustdoc;
pub mod visitor;

use std::path::Path;

use koffi_ir::CrateInterface;

use crate::{BindgenError, diagnostic::DiagnosticSink, meta::KoffiPackageMeta};

/// Full parse pipeline: syn pass -> rustdoc resolution -> [`CrateInterface`].
///
/// Returns the resolved IR and a [`DiagnosticSink`] containing all
/// diagnostics accumulated during both phases. The caller **must**:
///
/// 1. Call [`DiagnosticSink::emit`] to print every diagnostic to stderr.
/// 2. Call [`DiagnosticSink::has_errors`] to decide whether to treat the
///    result as a failure. Errors indicate malformed items that were skipped
///    or types that could not be resolved.
///
/// Truly fatal conditions (e.g. I/O errors, rustdoc failures) are returned
/// as `Err(BindgenError)` and abort immediately.
pub fn parse_crate(
    crate_path: &Path,
    _workspace_root: &Path,
    crate_name: String,
    version: String,
    meta: &KoffiPackageMeta,
    pkg_schemas: &[CrateInterface],
) -> Result<(CrateInterface, DiagnosticSink), BindgenError> {
    let crate_namespace = meta
        .namespace
        .clone()
        .unwrap_or_else(|| "generated".to_string());

    let partial = visitor::parse_syn(crate_path, crate_namespace, crate_name, version)?;
    let sink = partial.diagnostics.clone();

    let rustdoc_path = rustdoc::ensure_json(crate_path)?;
    let resolved = rustdoc::resolve_types(partial, &rustdoc_path, pkg_schemas)?;

    Ok((resolved, sink))
}

/// Syn-only parse pipeline (Phase 1 only, no rustdoc resolution).
///
/// Faster but less accurate: cross-crate type identities and schema hashes
/// will be missing or zero. Useful for rapid iteration; disable before
/// publishing or running codegen in CI.
pub fn parse_crate_syn_only(
    crate_path: &Path,
    crate_name: String,
    version: String,
    meta: &KoffiPackageMeta,
) -> Result<(CrateInterface, DiagnosticSink), BindgenError> {
    let crate_namespace = meta
        .namespace
        .clone()
        .unwrap_or_else(|| "generated".to_string());

    let visitor::PartialInterface {
        namespace,
        crate_name,
        version,
        structs,
        enums,
        functions,
        diagnostics: sink,
    } = visitor::parse_syn(crate_path, crate_namespace, crate_name, version)?;

    let ir = CrateInterface {
        namespace,
        crate_name,
        version,
        structs,
        enums,
        functions,
        imports: vec![],
    };

    Ok((ir, sink))
}
