pub mod config;
pub mod profile;

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
    shared_target_dir: Option<&Path>,
    config_args: &[String],
) -> anyhow::Result<(String, PathBuf)> {
    let (crate_name, target_dir) = crate_metadata(crate_path)?;

    debug!("building {} to extract its exports", crate_path.display());
    // The artifacts land where cargo actually build them: the shared target
    // dir when one is given, the crate's own otherwise.
    let build_dir = shared_target_dir.unwrap_or(&target_dir);
    let cdylib_path = build_host_cdylib(
        &crate_name,
        crate_path,
        build_dir,
        release,
        features,
        shared_target_dir,
        config_args,
    )?;

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
    shared_target_dir: Option<&Path>,
    config_args: &[String],
) -> anyhow::Result<PathBuf> {
    run_cargo_build(
        crate_dir,
        None,
        release,
        features,
        shared_target_dir,
        config_args,
    )?;

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

/// Cross-builds the crate for `triple` (native platform) and
/// returns the artifacts that exist afterwards. The build is one cargo
/// invocation covering both crate-types; only the artifacts matching the
/// requested type are looked up (`want_cdylib`/`want_staticlib`).
#[allow(clippy::too_many_arguments)]
pub fn build_for_target(
    crate_name: &str,
    crate_dir: &Path,
    target_dir: &Path,
    triple: &str,
    release: bool,
    features: &[&str],
    kinds: &[TargetKind],
    shared_target_dir: Option<&Path>,
    config_args: &[String],
) -> anyhow::Result<TargetArtifacts> {
    run_cargo_build(
        crate_dir,
        Some(triple),
        release,
        features,
        shared_target_dir,
        config_args,
    )?;

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

/// Cross-builds the glue for one android abi through `cargo ndk`, which
/// points the android triples at the NDK's clang toolchain (CC/AR/linker
/// env vars) so no per-target linker config is needed. Returns the staged
/// cdylib's path in the shared target dir; `build_and_stage` copies it
/// into `jniLibs/<abi>/`. cargo-ndk needs the rustup target for `triple`
/// and an NDK (`ANDROID_NDK_HOME` or `ANDROID_NDK_ROOT`, or an
/// `$ANDROID_SDK_ROOT/ndk` directory), both checked on the failure path so
/// the warning names the fix.
fn build_for_android(
    crate_name: &str,
    crate_dir: &Path,
    target_dir: &Path,
    abi: &str,
    triple: &str,
    release: bool,
    config_args: &[String],
) -> anyhow::Result<PathBuf> {
    let mut args = vec!["ndk", "-t", abi, "build"];
    if release {
        args.push("--release");
    }
    args.extend(["--features", "cabi,jni", "--target-dir"]);
    args.push(target_dir.to_str().ok_or_else(|| {
        anyhow::anyhow!("target dir is not valid UTF-8: {}", target_dir.display())
    })?);
    for config in config_args {
        args.push("--config");
        args.push(config);
    }

    let status = Command::new("cargo")
        .args(&args)
        .current_dir(crate_dir)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run cargo ndk for {}: {e}", crate_dir.display()))?;
    if !status.success() {
        let rustup = rustup_target_hint(triple);
        let ndk = android_ndk_env_hint();
        let ndk_hint = if ndk.is_empty() {
            String::new()
        } else {
            format!(
                " ({ndk}; export ANDROID_NDK_HOME or install the NDK via `sdkmanager --install ndk`)"
            )
        };
        anyhow::bail!(
            "cargo ndk build failed for {}{rustup}{ndk_hint}",
            crate_dir.display()
        );
    }

    Ok(target_dir
        .join(triple)
        .join(if release { "release" } else { "debug" })
        .join(platform_library_file_name(crate_name)))
}

/// Whether the `cargo ndk` subcommand exists (installed via
/// `cargo install cargo-ndk`).
fn cargo_ndk_available() -> bool {
    Command::new("cargo")
        .args(["ndk", "--version"])
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Probe for the NDK paths cargo-ndk discovers: its env vars, plus the
/// `~/Android/Sdk/ndk` convention. When none exist, a failed android build
/// gets a hint naming the fix; otherwise cargo-ndk's own error stands.
fn android_ndk_env_hint() -> &'static str {
    let env_set = [
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "ANDROID_NDK_PATH",
        "ANDROID_SDK_ROOT",
        "ANDROID_SDK_HOME",
        "ANDROID_HOME",
    ]
    .iter()
    .any(|var| std::env::var_os(var).is_some());
    let home_ndk = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .is_some_and(|home| home.join("Android/Sdk/ndk").is_dir());

    if env_set || home_ndk {
        ""
    } else {
        "no Android NDK found"
    }
}

fn run_cargo_build(
    crate_dir: &Path,
    triple: Option<&str>,
    release: bool,
    features: &[&str],
    shared_target_dir: Option<&Path>,
    config_args: &[String],
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
    for config in config_args {
        args.push("--config");
        args.push(config);
    }

    let mut cmd = Command::new("cargo");
    cmd.args(&args).current_dir(crate_dir);
    if let Some(dir) = shared_target_dir {
        // The glue crate is a separate cargo tree, but pointing it at the
        // source crate's target dir makes both share fingerprints, so `pack`
        // after `gen` (or a user's own `cargo build`) doesn't recompile the
        // source crate from scratch.
        cmd.env("CARGO_TARGET_DIR", dir);
    }

    let status = cmd
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run cargo for {}: {e}", crate_dir.display()))?;
    if !status.success() {
        let hint = match triple {
            Some(triple) => rustup_target_hint(triple),
            _ => String::new(),
        };
        anyhow::bail!("cargo build failed for {}{}", crate_dir.display(), hint);
    }

    Ok(())
}

/// What usually makes a cross build fail when the rustup target for
/// `triple` isn't installed, with the fix spelled out. `false` also covers
/// non-rustup setups, where the generic error stands on its own.
fn rustup_target_hint(triple: &str) -> String {
    if rustup_target_installed(triple) {
        String::new()
    } else {
        format!(
            " (the target toolchain is missing; run `rustup target add {triple}`, \
             or set `cross_compile = true` in the config to install it and try the cross build)"
        )
    }
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

/// Installs the rustup target for every configured cross target (native
/// platforms and android abis) that isn't installed yet, so a build can
/// cross-compile on a host that doesn't match the targets. Best-effort: a
/// missing rustup only warns - the cargo build then fails with its own hint.
fn ensure_cross_targets(config: &KoffiConfig, host: &str) -> anyhow::Result<()> {
    for (_, triple) in config
        .android_targets()
        .into_iter()
        .chain(config.native_targets())
    {
        if rustup_target_installed(triple) {
            debug!("cross target {triple} already installed");
            continue;
        }
        info!("installing rustup target {triple} for cross compilation (host: {host})");
        let installed = Command::new("rustup")
            .args(["target", "add", triple])
            .status()
            .is_ok_and(|s| s.success());
        if !installed {
            warn!(
                "could not install rustup target {triple}; install it manually or the build will fail"
            );
        }
    }

    Ok(())
}

/// The host target triple, read from `rustc -vV`. Only feeds cross-compile
/// decisions and log lines; cargo figures out what it needs on its own.
fn host_triple() -> String {
    Command::new("rustc")
        .args(["-vV"])
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8(out.stdout).ok().and_then(|v| {
                v.lines()
                    .find_map(|l| l.strip_prefix("host: ").map(str::to_owned))
            })
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// What usually supplies the C linker a cross build of `target` needs on
/// `host`. A cross build only fails this far when the linker is the problem
/// (the rustup target was already handled), so the hint names the install
/// command rather than cargo's bare message.
fn linker_hint(host: &str, target: &str) -> Option<&'static str> {
    if target.contains("darwin") {
        return Some("apple targets need a macOS SDK with clang (osxcross, or a real mac)");
    }
    match (triple_os(host), triple_os(target)) {
        ("linux", "windows") => {
            Some("install the mingw gcc, e.g. `sudo apt install gcc-mingw-w64-x86-64`")
        }
        ("linux", "linux") => {
            Some(
                "install the cross gcc for the target arch, e.g. `sudo apt install gcc-aarch64-linux-gnu`",
            )
        }
        ("windows", "linux") => {
            Some(
                "a linker for linux targets on Windows is impractical; cross-compile inside WSL or a linux container",
            )
        }
        ("macos", "windows") => Some("install the mingw gcc, e.g. `brew install mingw-w64`"),
        ("macos", "linux") => {
            Some(
                "install a linux cross toolchain (musl-cross) or cross-compile inside a linux container",
            )
        }
        _ => None,
    }
}

/// The OS a koffi target triple runs on, for the linker hints. The known
/// triples all spell the OS out verbatim (`-linux-`, `-windows-`, `darwin`),
/// so a substring scan beats parsing the irregular tuple layout.
fn triple_os(triple: &str) -> &'static str {
    if triple.contains("darwin") {
        "macos"
    } else if triple.contains("windows") {
        "windows"
    } else if triple.contains("linux") {
        "linux"
    } else {
        "unknown"
    }
}

/// Builds and stages everything the configured platforms need, then rewrites
/// the cinterop `.def` with absolute paths (the Kotlin compiler resolves
/// them relative to the def's directory, so relative ones silently miss).
///
/// `release` is the CLI's `-r` flag, and it rules both the source crate
/// build (`gen`) and the glue builds here, so the profiles always match.
/// The glue build writes into the source crate's target dir, so the
/// mandatory `cargo build` at `gen` time doubles as the first stage of
/// `pack`'s; same profiles mean shared fingerprints, and an unchanged
/// incremental `pack` stays under a second.
///
/// Layout, mirroring the plan:
/// - jvm: `resources@jvm/native/lib<ident>_glue.<ext>` (the JNI actual extracts
///   this from `/native/` at runtime)
/// - android: `jniLibs/<abi>/lib<ident>_glue.so` (best-effort: a missing
///   cargo-ndk/NDK warns and leaves the dir empty, the module still compiles)
/// - native: `libs/<KotlinPlatform>/lib<ident>_glue.a` (cinterop links it)
pub fn build_and_stage(
    dirs: &OutputDirs,
    source_crate_ident: &str,
    config: &KoffiConfig,
    release: bool,
    profile: &profile::SourceProfile,
) -> anyhow::Result<()> {
    let (glue_name, _) = crate_metadata(&dirs.rust_out_dir)?;
    let (_, source_target_dir) = crate_metadata(&dirs.crate_path)?;
    let host = host_triple();
    let glue_config_args = profile.glue_config_args();

    if config.cross_compile {
        ensure_cross_targets(config, &host)?;
    }

    if config.has(&TargetPlatform::Jvm) {
        let mut features = vec!["cabi"];
        if config.jvm_uses_jni() || config.has(&TargetPlatform::Android) {
            features.push("jni");
        }
        let (_, cdylib) = build_crate(
            &dirs.rust_out_dir,
            release,
            &features,
            Some(&source_target_dir),
            &glue_config_args,
        )?;
        stage(
            &cdylib,
            &dirs.kotlin_out_dir.join("resources@jvm").join("native"),
        )?;
    }

    if config.has(&TargetPlatform::Android) {
        if !cargo_ndk_available() {
            warn!(
                "android staging skipped: cargo-ndk is not installed (run `cargo install cargo-ndk`)"
            );
        } else {
            for (abi, triple) in config.android_targets() {
                match build_for_android(
                    &glue_name,
                    &dirs.rust_out_dir,
                    &source_target_dir,
                    abi,
                    triple,
                    release,
                    &glue_config_args,
                ) {
                    Ok(so) => {
                        stage(&so, &dirs.kotlin_out_dir.join("jniLibs").join(abi))?;
                    }
                    Err(e) => warn!("skipping android abi {abi}: {e:#}"),
                }
            }
        }
    }

    for (platform, triple) in config.native_targets() {
        if triple != host {
            if !config.cross_compile {
                continue;
            }

            info!("cross-compiling {platform} ({triple}) for host {host}");
        }

        let artifacts = build_for_target(
            &glue_name,
            &dirs.rust_out_dir,
            &source_target_dir,
            triple,
            release,
            &["cabi"],
            &[TargetKind::Staticlib],
            Some(&source_target_dir),
            &glue_config_args,
        )
        .map_err(|e| {
            let linker = linker_hint(&host, triple).map_or_else(String::new, |hint| {
                format!(" (the C linker for the target may be missing; {hint})")
            });

            anyhow::anyhow!("{e:#}{linker}")
        })?;

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

/// Rewrites the rendered `.def` in place: every `__KOFFI_LIBS__/<platform>`
/// becomes the absolute `<kotlin>/libs/<platform>`. The header line stays
/// relative - `headers = <crate>.h` resolves through the toolchain's
/// automatic `-I<kotlin>/cinterop@native/include`. `gen` keeps the
/// placeholders so snapshots stay stable; only `pack` absolutizes.
fn absolutize_def(
    kotlin_out_dir: &Path,
    crate_ident: &str,
    config: &KoffiConfig,
) -> anyhow::Result<()> {
    let def_path = kotlin_out_dir
        .join("cinterop@native")
        .join(format!("{crate_ident}.def"));
    if !def_path.exists() {
        return Ok(());
    }

    let kotlin_abs = absolute_path(kotlin_out_dir);
    let mut def = std::fs::read_to_string(&def_path)?;

    for platform in config.native_platforms() {
        let platform = platform.as_str();
        def = def.replace(
            &format!("__KOFFI_LIBS__/{platform}"),
            &kotlin_abs
                .join("libs")
                .join(platform)
                .to_string_lossy()
                .replace("\\", "/"),
        );
    }

    std::fs::write(&def_path, def)?;
    debug!("absolutized {} for cinterop", def_path.display());

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

/// Staticlib file name for the cross-compiled native artifacts.
#[must_use]
pub fn static_library_file_name(crate_name: &str) -> String {
    let crate_ident = crate_name.replace('-', "_");
    format!("lib{crate_ident}.a")
}
