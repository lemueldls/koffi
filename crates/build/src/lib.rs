pub mod config;

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use tracing::{debug, info, warn};

use crate::config::{KoffiConfig, TargetPlatform};

pub struct OutputDirs {
    pub crate_path: PathBuf,
    pub rust_out_dir: PathBuf,
    pub kotlin_out_dir: PathBuf,
}

/// The artifact pair cargo produces for a `--target <triple>` build. The
/// generated glue crate declares both cdylib and staticlib crate-types, so
/// either (or both) may exist; android wants the cdylib, native wants the
/// staticlib.
pub struct TargetArtifacts {
    pub cdylib: Option<PathBuf>,
    pub staticlib: Option<PathBuf>,
}

/// Which crate-types a cross build should produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Cdylib,
    Staticlib,
}

pub fn build_crate(
    crate_path: &Path,
    release: bool,
    features: &[&str],
) -> anyhow::Result<(String, PathBuf)> {
    let (crate_name, target_dir) = crate_metadata(crate_path)?;

    debug!("building {} to extract its exports", crate_path.display());
    let cdylib_path = build_host_cdylib(&crate_name, crate_path, &target_dir, release, features)?;

    Ok((crate_name, cdylib_path))
}

/// Reads the crate name and target directory from cargo metadata; the glue
/// crate lives in a separate tree, so this follows its own manifest rather
/// than the workspace's.
fn crate_metadata(crate_path: &Path) -> anyhow::Result<(String, PathBuf)> {
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

    Ok((
        target_crate.name.to_string(),
        metadata.target_directory.as_std_path().to_path_buf(),
    ))
}

pub fn build_host_cdylib(
    crate_name: &str,
    crate_dir: &Path,
    target_dir: &Path,
    release: bool,
    features: &[&str],
) -> anyhow::Result<PathBuf> {
    run_cargo_build(crate_dir, None, release, features)?;

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

/// Cross-builds the crate for `triple` (android abi or native platform) and
/// returns the artifacts that exist afterwards. The build is one cargo
/// invocation covering both crate-types; only the artifacts matching the
/// requested type are looked up (`want_cdylib`/`want_staticlib`).
pub fn build_for_target(
    crate_name: &str,
    crate_dir: &Path,
    target_dir: &Path,
    triple: &str,
    release: bool,
    features: &[&str],
    kinds: &[TargetKind],
) -> anyhow::Result<TargetArtifacts> {
    run_cargo_build(crate_dir, Some(triple), release, features)?;

    let profile_dir = target_dir
        .join(triple)
        .join(if release { "release" } else { "debug" });

    Ok(TargetArtifacts {
        cdylib: kinds
            .contains(&TargetKind::Cdylib)
            .then(|| profile_dir.join(platform_library_file_name(crate_name))),
        staticlib: kinds
            .contains(&TargetKind::Staticlib)
            .then(|| profile_dir.join(static_library_file_name(crate_name))),
    })
}

fn run_cargo_build(
    crate_dir: &Path,
    triple: Option<&str>,
    release: bool,
    features: &[&str],
) -> anyhow::Result<()> {
    let mut args = vec!["build"];
    if let Some(triple) = triple {
        args.push("--target");
        args.push(triple);
    }
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
    if !status.success() {
        let hint = match triple {
            Some(triple) if !rustup_target_installed(triple) => {
                format!(
                    " (the target toolchain is missing; run `rustup target add {triple}` and retry)"
                )
            }
            _ => String::new(),
        };
        anyhow::bail!("cargo build failed for {}{}", crate_dir.display(), hint);
    }

    Ok(())
}

/// Best-effort check whether a cross-compile target exists in the rustup
/// toolchain, so the error message can point at the fix. `false` also covers
/// non-rustup setups, where the generic error stands on its own.
fn rustup_target_installed(triple: &str) -> bool {
    Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .is_ok_and(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|l| l == triple)
        })
}

/// Builds and stages everything the configured platforms need, then rewrites
/// the cinterop `.def` with absolute paths (the Kotlin compiler resolves
/// them relative to the def's directory, so relative ones silently miss).
///
/// Layout, mirroring the plan:
/// - jvm: `resources@jvm/native/lib<ident>_glue.<ext>` (the JNI actual extracts
///   this from `/native/` at runtime)
/// - android: `jniLibs/<abi>/lib<ident>_glue.so` (best-effort: a missing
///   NDK/target warns and leaves the dir empty, the module still compiles)
/// - native: `libs/<KotlinPlatform>/lib<ident>_glue.a` (cinterop links it)
pub fn build_and_stage(
    dirs: &OutputDirs,
    source_crate_ident: &str,
    release: bool,
    config: &KoffiConfig,
) -> anyhow::Result<()> {
    let (glue_name, target_dir) = crate_metadata(&dirs.rust_out_dir)?;

    if config.has(&TargetPlatform::Jvm) {
        let mut features = vec!["cabi"];
        if config.jvm_uses_jni() || config.has(&TargetPlatform::Android) {
            features.push("jni");
        }
        let (_, cdylib) = build_crate(&dirs.rust_out_dir, release, &features)?;
        stage(
            &cdylib,
            &dirs.kotlin_out_dir.join("resources@jvm").join("native"),
        )?;
    }

    if config.has(&TargetPlatform::Android) {
        for (abi, triple) in config.android_targets() {
            match build_for_target(
                &glue_name,
                &dirs.rust_out_dir,
                &target_dir,
                triple,
                release,
                &["cabi", "jni"],
                &[TargetKind::Cdylib],
            ) {
                Ok(artifacts) => {
                    let so = artifacts.cdylib.expect("android wants the cdylib");
                    stage(&so, &dirs.kotlin_out_dir.join("jniLibs").join(abi))?;
                }
                Err(e) => warn!("skipping android abi {abi}: {e:#}"),
            }
        }
    }

    for (platform, triple) in config.native_targets() {
        let artifacts = build_for_target(
            &glue_name,
            &dirs.rust_out_dir,
            &target_dir,
            triple,
            release,
            &["cabi"],
            &[TargetKind::Staticlib],
        )?;
        let a = artifacts.staticlib.expect("native wants the staticlib");
        stage(&a, &dirs.kotlin_out_dir.join("libs").join(platform))?;
    }

    if !config.native_platforms().is_empty() {
        // The def/header file names use the underscored crate ident, not
        // the package name (hello-kotlin -> hello_kotlin).
        absolutize_def(
            &dirs.kotlin_out_dir,
            &source_crate_ident.replace('-', "_"),
            config,
        )?;
    }

    Ok(())
}

fn stage(src: &Path, dest_dir: &Path) -> anyhow::Result<()> {
    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("artifact {} has no file name", src.display()))?;
    debug!("staging {} -> {}", src.display(), dest_dir.display());
    std::fs::create_dir_all(dest_dir)?;
    std::fs::copy(src, dest_dir.join(file_name))?;

    Ok(())
}

/// Rewrites the rendered `.def` in place: `headers = <crate>.h` becomes the
/// absolute `<kotlin>/cinterop/<crate>.h`, and every
/// `__KOFFI_LIBS__/<platform>` becomes the absolute `<kotlin>/libs/<platform>`.
/// `gen` keeps the placeholders so snapshots stay stable; only `pack`
/// absolutizes.
fn absolutize_def(
    kotlin_out_dir: &Path,
    crate_ident: &str,
    config: &KoffiConfig,
) -> anyhow::Result<()> {
    let def_path = kotlin_out_dir
        .join("cinterop")
        .join(format!("{crate_ident}.def"));
    if !def_path.exists() {
        return Ok(());
    }

    let kotlin_abs = absolute_path(kotlin_out_dir);
    let header_abs = kotlin_abs.join("cinterop").join(format!("{crate_ident}.h"));
    let mut def = std::fs::read_to_string(&def_path)?;
    def = def.replace(
        &format!("headers = {crate_ident}.h"),
        &format!("headers = {}", header_abs.display()),
    );

    for platform in config.native_platforms() {
        let platform = platform.as_str();
        def = def.replace(
            &format!("__KOFFI_LIBS__/{platform}"),
            &kotlin_abs.join("libs").join(platform).to_string_lossy(),
        );
    }

    std::fs::write(&def_path, def)?;
    info!("absolutized {} for cinterop", def_path.display());

    Ok(())
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("failed to read current directory")
            .join(path)
    }
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

/// Staticlib file name for the cross-compiled native artifacts. The cinterop
/// def hardcodes `lib<ident>_glue.a`, which matches every unix target; the
/// mingw case (`.lib`) is out of M0's scope.
#[must_use]
pub fn static_library_file_name(crate_name: &str) -> String {
    let crate_ident = crate_name.replace('-', "_");
    if cfg!(target_os = "windows") {
        format!("{crate_ident}.lib")
    } else {
        format!("lib{crate_ident}.a")
    }
}
