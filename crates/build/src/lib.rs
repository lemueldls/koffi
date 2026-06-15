//! Build-time integration helper for koffi.
//!
//! Add this to your crate's `build.rs`:
//!
//! ```rust,no_run
//! fn main() {
//!     koffi_build::build().unwrap();
//! }
//! ```
//!
//! Or with explicit configuration:
//!
//! ```rust,no_run
//! fn main() {
//!     koffi_build::Builder::from_env()
//!         .out_dir("generated")
//!         .jvm_only()
//!         .build()
//!         .unwrap();
//! }
//! ```
//!
//! ## What this does
//!
//! 1. Reads `[package.metadata.koffi]` from every transitive dependency via
//!    `cargo_metadata`.
//! 2. Runs the two-phase parse pipeline (syn + rustdoc JSON) on each koffi-
//!    aware crate in topological order, passing earlier crates' schemas as
//!    `pkg_schemas` to later ones.
//! 3. Generates Kotlin expect/actual files, a Rust JNI glue crate, a Rust C-ABI
//!    glue crate, a C header, and a cinterop `.def` file.
//! 4. Copies the koffi-runtime Kotlin sources into the output directory.
//! 5. Compiles the native glue libraries for the requested target set and
//!    places them in the correct output subdirectories.
//! 6. Emits `cargo:rerun-if-changed` directives so incremental builds work.

#![allow(clippy::needless_doctest_main)]

use std::{
    fs,
    path::{Path, PathBuf},
};

pub use koffi_bindgen::{BindgenError, build_steps::TargetPlatforms as Targets};
use koffi_bindgen::{
    build_steps::BuildSteps,
    codegen::{self, BindingPackage},
    meta::{KoffiPackage, collect_koffi_packages},
    parser,
};
use koffi_ir::CrateInterface;

/// Run the full koffi code-generation pipeline from a `build.rs` context,
/// using default settings derived from `CARGO_*` environment variables.
///
/// Equivalent to `Builder::from_env().build()`.
pub fn build() -> Result<(), BindgenError> {
    Builder::from_env().build()
}

/// Configures and runs the koffi code-generation pipeline.
pub struct Builder {
    /// Directory containing the crate's `Cargo.toml`.
    manifest_dir: PathBuf,
    /// Where to write all generated output. Default:
    /// `<manifest_dir>/generated`.
    out_dir: PathBuf,
    /// Workspace root directory.
    workspace_root: PathBuf,
    /// Which native targets to compile.
    targets: Targets,
    /// Whether to enable release mode when compiling libraries.
    release: bool,
    /// Whether to emit rustdoc-JSON-based type resolution (Phase 2).
    /// Disable only for fast iteration; schema hashes will be zero.
    full_parse: bool,
}

impl Builder {
    /// Create a `Builder` from `CARGO_*` environment variables set by Cargo
    /// when running a `build.rs` script.
    ///
    /// | Variable                                        | Used for       |
    /// |-------------------------------------------------|----------------|
    /// | `CARGO_MANIFEST_DIR`                            | Crate root     |
    /// | `CARGO_WORKSPACE_DIR` (if set) or auto-detected | Workspace root |
    pub fn from_env() -> Self {
        let manifest_dir = PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR must be set (call from build.rs)"),
        );

        // Try CARGO_WORKSPACE_DIR (set by some cargo versions), else walk up
        // from manifest_dir until we find a workspace Cargo.toml.
        let workspace_root = std::env::var("CARGO_WORKSPACE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| detect_workspace_root(&manifest_dir));

        let out_dir = manifest_dir.join("generated");

        Self {
            manifest_dir,
            out_dir,
            workspace_root,
            targets: Targets::default(),
            release: false,
            full_parse: true,
        }
    }

    /// Override the output directory.
    #[must_use]
    pub fn out_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.out_dir = dir.into();
        self
    }

    /// Set which native targets to compile.
    #[must_use]
    pub fn targets(mut self, t: Targets) -> Self {
        self.targets = t;
        self
    }

    /// Set release mode when compiling libraries.
    #[must_use]
    pub const fn release(mut self, release: bool) -> Self {
        self.release = release;
        self
    }

    /// Only compile for the host jvm. Shorthand for
    /// `.targets(Targets::jvm_only())`.
    #[must_use]
    pub fn jvm_only(self) -> Self {
        self.targets(Targets::jvm_only())
    }

    /// Disable Phase 2 (rustdoc JSON) type resolution.
    ///
    /// With Phase 2 disabled, type resolution falls back to Phase 1 (syn-only)
    /// results. Cross-crate type identities and schema hashes will be missing
    /// or zero. Use only for rapid iteration; disable before publishing.
    #[must_use]
    pub const fn skip_rustdoc(mut self) -> Self {
        self.full_parse = false;
        self
    }

    /// Run the full code-generation and compilation pipeline.
    pub fn build(self) -> Result<(), BindgenError> {
        emit_rerun_directives(&self.manifest_dir);

        let manifest = self.manifest_dir.join("Cargo.toml");
        let out_dir = std::path::absolute(&self.out_dir)?;

        // Collect all koffi-aware crates in the dependency graph, including
        // the root crate itself if it has [package.metadata.koffi].
        let packages = collect_koffi_packages(&manifest)?;

        if packages.is_empty() {
            eprintln!(
                "koffi-build: no [package.metadata.koffi] found in {} or its dependencies",
                manifest.display(),
            );

            return Ok(());
        }

        // Parse crates in topological order so each crate can see dep schemas.
        let mut pkg_schemas: Vec<CrateInterface> = Vec::new();
        let mut parsed_packages: Vec<(KoffiPackage, CrateInterface)> = Vec::new();

        for pkg in packages {
            let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));

            let ir = if self.full_parse {
                parser::parse_crate(
                    crate_path,
                    &self.workspace_root,
                    pkg.name.clone(),
                    pkg.version.clone(),
                    &pkg.koffi_meta,
                    &pkg_schemas,
                )?
            } else {
                // Phase 1 only. Faster, less accurate.
                parser::parse_crate_syn_only(
                    crate_path,
                    pkg.name.clone(),
                    pkg.version.clone(),
                    &pkg.koffi_meta,
                )?
            };

            // Optionally emit schema.json next to the crate (for plugin crates
            // that want to check it in).
            if pkg.koffi_meta.schema.is_some() {
                let schema_path = crate_path.join("koffi/schema.json");
                if let Some(parent) = schema_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                codegen::emit_schema(&ir, &schema_path)?;
            }

            pkg_schemas.push(ir.clone());
            parsed_packages.push((pkg, ir));
        }

        let root_index = parsed_packages
            .iter()
            .position(|(pkg, _)| pkg.is_root)
            .unwrap_or_else(|| parsed_packages.len().saturating_sub(1));
        let (root_pkg, root_ir) = &parsed_packages[root_index];
        let target_platforms = root_pkg
            .koffi_meta
            .target_platforms
            .clone()
            .unwrap_or_else(|| self.targets.clone());
        let binding_packages = parsed_packages
            .iter()
            .map(|(pkg, ir)| {
                let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));
                BindingPackage { ir, crate_path }
            })
            .collect::<Vec<_>>();

        codegen::generate_package_set(
            &binding_packages,
            &root_ir.crate_name,
            &root_ir.version,
            &out_dir,
            &target_platforms,
        )?;

        // Compile native libraries after all code generation is done, so that
        // the generated Rust glue crate sees all types from all plugins.
        let crate_ident = root_ir.crate_name.replace('-', "_");
        let steps = BuildSteps {
            crate_path: root_pkg
                .manifest_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
            out_dir: out_dir.clone(),
            glue_path: out_dir.join("rust"),
            crate_ident: crate_ident.clone(),
            lib_name: format!("{crate_ident}_koffi_glue"),
        };
        steps.run_targets(&target_platforms, self.release)?;

        Ok(())
    }
}

/// Emit `cargo:rerun-if-changed` for the crate source tree so that Cargo
/// re-runs `build.rs` whenever a Rust source file changes.
fn emit_rerun_directives(manifest_dir: &Path) {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/");

    // Also rerun if any `.rs` file inside src/ changes (belt-and-suspenders).
    if let Ok(entries) = walkdir_rs(manifest_dir.join("src")) {
        for path in entries {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

/// Walk a directory and collect all `.rs` file paths.
fn walkdir_rs(dir: impl AsRef<Path>) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_rs_inner(dir.as_ref(), &mut out)?;

    Ok(out)
}

fn walk_rs_inner(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_rs_inner(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }

    Ok(())
}

/// Walk up from `start` until a `Cargo.toml` containing `[workspace]` is found.
fn detect_workspace_root(start: &Path) -> PathBuf {
    let mut current = start.to_path_buf();

    loop {
        let candidate = current.join("Cargo.toml");
        if candidate.exists()
            && let Ok(contents) = fs::read_to_string(&candidate)
            && contents.contains("[workspace]")
        {
            return current;
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return start.to_path_buf(),
        }
    }
}
