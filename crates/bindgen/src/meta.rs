use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use cargo_metadata::{Metadata, MetadataCommand, Package, PackageId};
use koffi_ir::CrateInterface;
use serde::{Deserialize, Serialize};

use crate::BindgenError;

/// Contents of `[package.metadata.koffi]` in any koffi-aware crate's
/// Cargo.toml.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct KoffiPackageMeta {
    /// Default Kotlin namespace for all exported items in this crate.
    /// Example: "rs.koffi.camera"
    pub namespace: Option<String>,

    /// Path to the pre-generated type schema, relative to the crate root.
    /// Checked into source for published plugin crates so that downstream
    /// users don't need to run bindgen to get type resolution.
    /// Default: "koffi/schema.json"
    pub schema: Option<PathBuf>,

    /// Path to pre-generated Kotlin sources, relative to the crate root.
    /// When present, bindgen copies these instead of regenerating.
    /// Example: "koffi/kotlin"
    pub kotlin_prebuilt: Option<PathBuf>,

    /// Feature-gated additional exports.
    #[serde(default)]
    pub features: HashMap<String, KoffiFeatureMeta>,

    /// Minimum koffi-bindgen version required to process this crate.
    pub min_bindgen_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KoffiFeatureMeta {
    /// Additional type names enabled by this feature.
    pub extra_types: Vec<String>,
    /// Additional function names enabled by this feature.
    pub extra_functions: Vec<String>,
}

impl KoffiPackageMeta {
    /// Resolved schema path (defaults to "koffi/schema.json").
    #[must_use]
    pub fn schema_path(&self, crate_root: &Path) -> PathBuf {
        crate_root.join(
            self.schema
                .clone()
                .unwrap_or_else(|| PathBuf::from("koffi/schema.json")),
        )
    }
}

#[derive(Debug)]
pub struct DepInfo {
    pub package: Package,
    pub workspace_root: PathBuf,
    pub koffi_meta: KoffiPackageMeta,
    pub schema: Option<CrateInterface>, // None if no schema.json exists
}

/// Walk the full dependency graph of the root crate and collect all
/// packages that carry `[package.metadata.koffi]`.
pub fn collect_koffi_deps(root_manifest: &Path) -> Result<Vec<DepInfo>, BindgenError> {
    let metadata = MetadataCommand::new().manifest_path(root_manifest).exec()?;

    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or(BindgenError::NoResolveGraph)?;

    let root_id = resolve.root.as_ref().ok_or(BindgenError::NoRootPackage)?;

    let mut visited = HashSet::new();
    let mut result = Vec::new();

    collect_recursive(root_id, &metadata, &mut visited, &mut result)?;

    Ok(result)
}

fn collect_recursive(
    pkg_id: &PackageId,
    metadata: &Metadata,
    visited: &mut HashSet<PackageId>,
    out: &mut Vec<DepInfo>,
) -> Result<(), BindgenError> {
    if !visited.insert(pkg_id.clone()) {
        return Ok(());
    }

    let pkg = metadata
        .packages
        .iter()
        .find(|p| &p.id == pkg_id)
        .ok_or_else(|| BindgenError::PackageNotFound(pkg_id.to_string()))?;

    if let Some(koffi_meta) = extract_koffi_meta(pkg) {
        let crate_root = pkg.manifest_path.parent().unwrap().as_std_path();
        let schema = load_schema(&koffi_meta, crate_root);
        let workspace_root = metadata.workspace_root.clone().into_std_path_buf();
        out.push(DepInfo {
            package: pkg.clone(),
            workspace_root,
            koffi_meta,
            schema,
        });
    }

    // Recurse into dependencies
    if let Some(resolve) = &metadata.resolve
        && let Some(node) = resolve.nodes.iter().find(|n| &n.id == pkg_id) {
            for dep_id in &node.dependencies {
                collect_recursive(dep_id, metadata, visited, out)?;
            }
        }

    Ok(())
}

fn extract_koffi_meta(pkg: &Package) -> Option<KoffiPackageMeta> {
    let koffi_value = pkg.metadata.get("koffi")?;
    serde_json::from_value::<KoffiPackageMeta>(koffi_value.clone()).ok()
}

fn load_schema(meta: &KoffiPackageMeta, crate_root: &Path) -> Option<CrateInterface> {
    let path = meta.schema_path(crate_root);
    let json = std::fs::read_to_string(path).ok()?;

    serde_json::from_str(&json).ok()
}
