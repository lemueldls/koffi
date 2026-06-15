use std::{collections::HashSet, fmt, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::BindgenError;

/// Kotlin target platforms supported by Koffi generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetPlatform {
    Jvm,
    Android,
    IosArm64,
    IosSimulatorArm64,
    IosX64,
    WasmJs,
    LinuxArm64,
    LinuxX64,
    MacosArm64,
    MacosX64,
    MingwX64,
}

impl TargetPlatform {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Jvm => "jvm",
            Self::Android => "android",
            Self::IosArm64 => "iosArm64",
            Self::IosSimulatorArm64 => "iosSimulatorArm64",
            Self::IosX64 => "iosX64",
            Self::WasmJs => "wasmJs",
            Self::LinuxArm64 => "linuxArm64",
            Self::LinuxX64 => "linuxX64",
            Self::MacosArm64 => "macosArm64",
            Self::MacosX64 => "macosX64",
            Self::MingwX64 => "mingwX64",
        }
    }
}

/// Target platform list shared by bindgen, native build steps, and the module config.
#[derive(Debug, Clone)]
pub struct TargetPlatforms {
    platforms: Vec<TargetPlatform>,
}

impl Default for TargetPlatforms {
    fn default() -> Self {
        Self::new([
            TargetPlatform::Jvm,
            TargetPlatform::Android,
            #[cfg(target_os = "macos")]
            TargetPlatform::IosArm64,
            #[cfg(target_os = "macos")]
            TargetPlatform::IosSimulatorArm64,
            #[cfg(target_os = "macos")]
            TargetPlatform::IosX64,
        ])
    }
}

impl TargetPlatforms {
    /// Every platform name understood by metadata aliases.
    #[must_use]
    pub fn all() -> Self {
        Self::new([
            TargetPlatform::Jvm,
            TargetPlatform::Android,
            TargetPlatform::IosArm64,
            TargetPlatform::IosSimulatorArm64,
            TargetPlatform::IosX64,
            TargetPlatform::WasmJs,
            TargetPlatform::LinuxArm64,
            TargetPlatform::LinuxX64,
            TargetPlatform::MacosArm64,
            TargetPlatform::MacosX64,
            TargetPlatform::MingwX64,
        ])
    }

    /// Only compile/generate the host-native JVM library.
    #[must_use]
    pub fn jvm_only() -> Self {
        Self::new([TargetPlatform::Jvm])
    }

    #[must_use]
    pub fn new(platforms: impl IntoIterator<Item = TargetPlatform>) -> Self {
        let mut seen = HashSet::new();
        let mut deduped = Vec::new();

        for platform in platforms {
            if seen.insert(platform) {
                deduped.push(platform);
            }
        }

        Self { platforms: deduped }
    }

    pub fn from_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self, String> {
        let mut platforms = Vec::new();

        for name in names {
            let name = name.as_ref();
            match normalize_platform_name(name).as_str() {
                "all" => platforms.extend(Self::all().platforms),
                "desktop" | "native" => {
                    platforms.extend([
                        TargetPlatform::LinuxArm64,
                        TargetPlatform::LinuxX64,
                        TargetPlatform::MacosArm64,
                        TargetPlatform::MacosX64,
                        TargetPlatform::MingwX64,
                    ]);
                }
                "ios" => {
                    platforms.extend([
                        TargetPlatform::IosArm64,
                        TargetPlatform::IosSimulatorArm64,
                        TargetPlatform::IosX64,
                    ]);
                }
                "jvm" => platforms.push(TargetPlatform::Jvm),
                "android" => platforms.push(TargetPlatform::Android),
                "iosarm64" => platforms.push(TargetPlatform::IosArm64),
                "iossimulatorarm64" => platforms.push(TargetPlatform::IosSimulatorArm64),
                "iosx64" => platforms.push(TargetPlatform::IosX64),
                "wasmjs" | "wasm" | "web" => platforms.push(TargetPlatform::WasmJs),
                "linuxarm64" => platforms.push(TargetPlatform::LinuxArm64),
                "linuxx64" => platforms.push(TargetPlatform::LinuxX64),
                "macosarm64" => platforms.push(TargetPlatform::MacosArm64),
                "macosx64" => platforms.push(TargetPlatform::MacosX64),
                "mingwx64" | "windowsx64" => platforms.push(TargetPlatform::MingwX64),
                _ => return Err(format!("unknown Koffi target platform `{name}`")),
            }
        }

        Ok(Self::new(platforms))
    }

    #[must_use]
    pub fn platforms(&self) -> Vec<&'static str> {
        self.platforms
            .iter()
            .map(|platform| platform.name())
            .collect()
    }

    #[must_use]
    pub fn contains(&self, platform: TargetPlatform) -> bool {
        self.platforms.contains(&platform)
    }

    #[must_use]
    pub fn ios_targets(&self) -> Vec<(&'static str, &'static str)> {
        let mut targets = Vec::new();

        if self.contains(TargetPlatform::IosArm64) {
            targets.push(("aarch64-apple-ios", "ios-aarch64"));
        }
        if self.contains(TargetPlatform::IosSimulatorArm64) {
            targets.push(("aarch64-apple-ios-sim", "ios-simulator-aarch64"));
        }
        if self.contains(TargetPlatform::IosX64) {
            targets.push(("x86_64-apple-ios", "ios-x86_64"));
        }

        targets
    }

    #[must_use]
    pub fn native_targets(&self) -> Vec<(&'static str, &'static str)> {
        let mut targets = Vec::new();

        if self.contains(TargetPlatform::LinuxArm64) {
            targets.push(("aarch64-unknown-linux-gnu", "linux-aarch64"));
        }
        if self.contains(TargetPlatform::LinuxX64) {
            targets.push(("x86_64-unknown-linux-gnu", "linux-x86_64"));
        }
        if self.contains(TargetPlatform::MacosArm64) {
            targets.push(("aarch64-apple-darwin", "macos-aarch64"));
        }
        if self.contains(TargetPlatform::MacosX64) {
            targets.push(("x86_64-apple-darwin", "apple-x86_64"));
        }
        if self.contains(TargetPlatform::MingwX64) {
            targets.push(("x86_64-pc-windows-gnu", "windows-x86_64"));
        }

        targets
    }
}

impl Serialize for TargetPlatforms {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        self.platforms().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TargetPlatforms {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let names = Vec::<String>::deserialize(deserializer)?;
        Self::from_names(names).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for TargetPlatforms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.platforms().join(", "))
    }
}

fn normalize_platform_name(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '-' && *c != '_' && !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The set of native build steps performed after source codegen.
/// Each step is skipped if the corresponding target feature is disabled.
#[derive(Debug)]
pub struct BuildSteps {
    pub crate_path: PathBuf,
    pub out_dir: PathBuf,
    pub glue_path: PathBuf, // path to generated/rust/
    pub crate_ident: String,
    pub lib_name: String,
}

impl BuildSteps {
    pub fn run_targets(&self, targets: &TargetPlatforms) -> Result<(), BindgenError> {
        if targets.contains(TargetPlatform::Android) {
            self.run_android()?;
        }
        if targets.contains(TargetPlatform::Jvm) {
            self.run_jvm()?;
        }
        self.run_ios_targets(&targets.ios_targets())?;
        self.run_native_targets(&targets.native_targets())?;

        Ok(())
    }

    pub fn run_android(&self) -> Result<(), BindgenError> {
        let targets = [
            ("aarch64-linux-android", "arm64-v8a"),
            ("armv7-linux-androideabi", "armeabi-v7a"),
            ("x86_64-linux-android", "x86_64"),
            ("i686-linux-android", "x86"),
        ];

        for (target, abi) in targets {
            info!("Building Android target {target} for ABI {abi}");
            let so = self.cargo_build_cdylib_ndk(target, "android")?;
            let dest = self
                .out_dir
                .join("kotlin/jniLibs")
                .join(abi)
                .join(format!("lib{}.so", self.lib_name));
            fs::create_dir_all(dest.parent().expect("should have a parent directory"))?;
            fs::copy(&so, &dest)?;
        }

        Ok(())
    }

    pub fn run_jvm(&self) -> Result<(), BindgenError> {
        let (target, classifier, prefix, ext) = host_triple();
        let lib = self.cargo_build_cdylib(target, "jvm", prefix, ext)?;
        let dest = self
            .out_dir
            .join("kotlin/resources@jvm/natives")
            .join(classifier)
            .join(format!("{prefix}{}{ext}", self.lib_name));
        fs::create_dir_all(dest.parent().expect("should have a parent directory"))?;
        fs::copy(&lib, &dest)?;

        Ok(())
    }

    fn run_native_targets(&self, targets: &[(&str, &str)]) -> Result<(), BindgenError> {
        for (target, slice) in targets {
            info!("Building native target {target} for {slice}");
            let lib = self.cargo_build_staticlib(target, "native")?;
            let dest = self
                .out_dir
                .join("kotlin/cinterop")
                .join(slice)
                .join(format!("lib{}.a", self.lib_name));
            fs::create_dir_all(dest.parent().expect("should have a parent directory"))?;
            fs::copy(&lib, &dest)?;
        }

        Ok(())
    }

    fn run_ios_targets(&self, targets: &[(&str, &str)]) -> Result<(), BindgenError> {
        for (target, slice) in targets {
            info!("Building iOS target {target} for {slice}");
            let lib = self.cargo_build_staticlib(target, "native")?;
            let dest = self
                .out_dir
                .join("kotlin/cinterop")
                .join(slice)
                .join(format!("lib{}.a", self.lib_name));
            fs::create_dir_all(dest.parent().expect("should have a parent directory"))?;
            fs::copy(&lib, &dest)?;
        }

        Ok(())
    }

    fn cargo_build_cdylib(
        &self,
        target: &str,
        feature: &str,
        prefix: &str,
        ext: &str,
    ) -> Result<PathBuf, BindgenError> {
        let status = std::process::Command::new("cargo")
            .args([
                "build",
                "--release",
                "--manifest-path",
                &self.glue_path.join("Cargo.toml").display().to_string(),
                "--target",
                target,
                "--features",
                feature,
            ])
            .status()?;
        if !status.success() {
            return Err(BindgenError::CargoBuildFailed(target.into()));
        }

        // Locate the artifact
        let artifact = self
            .glue_path
            .join("target")
            .join(target)
            .join("release")
            .join(format!("{prefix}{}{ext}", self.lib_name.replace('-', "_")));

        Ok(artifact)
    }

    fn cargo_build_cdylib_ndk(&self, target: &str, feature: &str) -> Result<PathBuf, BindgenError> {
        let status = std::process::Command::new("cargo")
            .args([
                "ndk",
                "--target",
                target,
                "build",
                "--release",
                "--manifest-path",
                &self.glue_path.join("Cargo.toml").display().to_string(),
                "--features",
                feature,
            ])
            .status()?;
        if !status.success() {
            return Err(BindgenError::CargoBuildFailed(target.into()));
        }

        // Locate the artifact
        let artifact = self
            .glue_path
            .join("target")
            .join(target)
            .join("release")
            .join(format!("lib{}.so", self.lib_name.replace('-', "_")));

        Ok(artifact)
    }

    fn cargo_build_staticlib(&self, target: &str, feature: &str) -> Result<PathBuf, BindgenError> {
        let status = std::process::Command::new("cargo")
            .args([
                "build",
                "--release",
                "--manifest-path",
                &self.glue_path.join("Cargo.toml").display().to_string(),
                "--target",
                target,
                "--features",
                feature,
            ])
            .status()?;
        if !status.success() {
            return Err(BindgenError::CargoBuildFailed(target.into()));
        }

        let artifact = self
            .glue_path
            .join("target")
            .join(target)
            .join("release")
            .join(format!("lib{}.a", self.lib_name.replace('-', "_")));

        Ok(artifact)
    }
}

const fn host_triple() -> (&'static str, &'static str, &'static str, &'static str) {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return ("aarch64-apple-darwin", "darwin-aarch64", "lib", ".dylib");
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return ("x86_64-apple-darwin", "darwin-x86_64", "lib", ".dylib");
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return ("x86_64-unknown-linux-gnu", "linux-x86_64", "lib", ".so");
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return ("aarch64-unknown-linux-gnu", "linux-aarch64", "lib", ".so");
    #[cfg(target_os = "windows")]
    return ("x86_64-pc-windows-msvc", "windows-x86_64", "", ".dll");
}
