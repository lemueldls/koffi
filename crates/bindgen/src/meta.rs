use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use cargo_metadata::{Metadata, MetadataCommand, Package, PackageId};
use koffi_ir::CrateInterface;
use serde::{Deserialize, Serialize};

use crate::{BindgenError, build_steps::TargetPlatforms};

/// Contents of `[package.metadata.koffi]` in any koffi-aware crate's Cargo.toml.
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

    /// Feature-gated additional exports.
    #[serde(default)]
    pub features: HashMap<String, KoffiFeatureMeta>,

    /// Minimum koffi-bindgen version required to process this crate.
    pub min_bindgen_version: Option<String>,

    /// Kotlin target platforms to generate and build for.
    ///
    /// Example: `target-platforms = ["jvm", "android", "iosArm64"]`
    pub target_platforms: Option<TargetPlatforms>,
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
pub struct KoffiPackage {
    pub name: String,
    pub version: String,
    pub manifest_path: PathBuf,
    pub workspace_root: PathBuf,
    pub koffi_meta: KoffiPackageMeta,
    pub schema: Option<CrateInterface>, // None if no schema.json exists
    pub is_root: bool,
}

/// Walk the full dependency graph of the root crate and collect all
/// packages that carry `[package.metadata.koffi]`.
pub fn collect_koffi_packages(root_manifest: &Path) -> Result<Vec<KoffiPackage>, BindgenError> {
    let metadata = MetadataCommand::new().manifest_path(root_manifest).exec()?;

    let root_pkg = metadata.root_package().ok_or(BindgenError::NoRootPackage)?;

    let mut visited = HashSet::new();
    let mut result = Vec::new();

    collect_recursive(root_pkg, true, &metadata, &mut visited, &mut result)?;

    Ok(result)
}

fn collect_recursive(
    pkg: &Package,
    is_root: bool,
    metadata: &Metadata,
    visited: &mut HashSet<PackageId>,
    out: &mut Vec<KoffiPackage>,
) -> Result<(), BindgenError> {
    if let Some(koffi_meta) = extract_koffi_meta(pkg)? {
        // Recurse into dependencies first so callers can parse dependencies before
        // dependents and pass their schemas forward.
        if let Some(resolve) = &metadata.resolve
            && let Some(node) = resolve.nodes.iter().find(|n| n.id == pkg.id)
        {
            for pkg_id in &node.dependencies {
                if visited.insert(pkg_id.clone()) {
                    collect_recursive(&metadata[pkg_id], false, metadata, visited, out)?;
                }
            }
        }

        let crate_root = pkg
            .manifest_path
            .parent()
            .expect("manifest should have a parent directory")
            .as_std_path();
        let schema = load_schema(&koffi_meta, crate_root);
        let name = pkg.name.to_string();
        let version = pkg.version.to_string();
        let manifest_path = pkg.manifest_path.clone().into_std_path_buf();
        let workspace_root = metadata.workspace_root.clone().into_std_path_buf();

        out.push(KoffiPackage {
            name,
            version,
            manifest_path,
            workspace_root,
            koffi_meta,
            schema,
            is_root,
        });
    }

    Ok(())
}

fn extract_koffi_meta(pkg: &Package) -> Result<Option<KoffiPackageMeta>, BindgenError> {
    let value = pkg.metadata.get("koffi");

    match value {
        Some(v) => {
            let meta = serde_json::from_value::<KoffiPackageMeta>(v.clone())?;
            Ok(Some(meta))
        }
        None => Ok(None),
    }
}

fn load_schema(meta: &KoffiPackageMeta, crate_root: &Path) -> Option<CrateInterface> {
    let path = meta.schema_path(crate_root);
    let json = std::fs::read_to_string(path).ok()?;

    facet_json::from_str(&json).ok()
}
