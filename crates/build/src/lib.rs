use std::{
    path::{Path, PathBuf},
    process::Command,
};

use tracing::debug;

pub struct OutputDirs {
    pub crate_path: PathBuf,
    pub rust_out_dir: PathBuf,
    pub kotlin_out_dir: PathBuf,
}

pub fn build_crate(
    crate_path: &Path,
    release: bool,
    features: &[&str],
) -> anyhow::Result<(String, PathBuf)> {
    let manifest_path = crate_path
        .join("Cargo.toml")
        .canonicalize()
        .expect("failed to canonicalize manifest path");
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec()
        .expect("failed to read cargo metadata");
    let target_crate = metadata
        .packages
        .iter()
        .find(|package| package.manifest_path.as_path() == manifest_path)
        .expect("failed to find target crate");
    let crate_name = target_crate.name.to_string();
    let target_dir = metadata.target_directory.as_std_path();

    debug!("building {} to extract its exports", crate_path.display());
    let cdylib_path = build_host_cdylib(&crate_name, crate_path, target_dir, release, features)?;

    Ok((crate_name, cdylib_path))
}

pub fn build_host_cdylib(
    crate_name: &str,
    crate_dir: &Path,
    target_dir: &Path,
    release: bool,
    features: &[&str],
) -> anyhow::Result<PathBuf> {
    let mut args = vec!["build"];
    if release {
        args.push("--release");
    }
    for feature in features {
        args.push("--features");
        args.push(feature);
    }

    let status = Command::new("cargo")
        .args(&args)
        .current_dir(crate_dir)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run cargo for {}: {e}", crate_dir.display()))?;
    anyhow::ensure!(
        status.success(),
        "cargo build failed for {}",
        crate_dir.display()
    );

    let cdylib_path = target_dir
        .join(if release { "release" } else { "debug" })
        .join(platform_library_file_name(crate_name));
    anyhow::ensure!(
        cdylib_path.exists(),
        "expected {} after a successful build but didn't find it",
        cdylib_path.display()
    );

    Ok(cdylib_path)
}

pub fn build_and_stage_cabi(dirs: &OutputDirs, release: bool) -> anyhow::Result<()> {
    let (_, cdylib_path) = build_crate(&dirs.rust_out_dir, release, &["cabi"])?;

    let dest_dir = dirs.kotlin_out_dir.join("resources@jvm").join("native");
    std::fs::create_dir_all(&dest_dir)?;
    std::fs::copy(
        &cdylib_path,
        dest_dir.join(cdylib_path.file_name().unwrap()),
    )?;

    Ok(())
}

#[must_use]
pub fn platform_library_file_name(crate_name: &str) -> String {
    let crate_ident = crate_name.replace('-', "_");

    if cfg!(target_os = "macos") {
        format!("lib{crate_ident}.dylib")
    } else if cfg!(target_os = "windows") {
        format!("{crate_ident}.dll")
    } else {
        format!("lib{crate_ident}.so")
    }
}
