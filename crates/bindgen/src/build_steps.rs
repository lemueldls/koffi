use std::{fs, path::PathBuf};

use crate::BindgenError;

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
    pub fn run_android(&self) -> Result<(), BindgenError> {
        let targets = [
            ("aarch64-linux-android", "arm64-v8a"),
            ("armv7-linux-androideabi", "armeabi-v7a"),
            ("x86_64-linux-android", "x86_64"),
            ("i686-linux-android", "x86"),
        ];

        for (target, abi) in targets {
            let so = self.cargo_build_cdylib(target, "android", "lib", ".so")?;
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

    pub fn run_ios(&self) -> Result<(), BindgenError> {
        let targets = [
            ("aarch64-apple-ios", "iosArm64"),
            ("aarch64-apple-ios-sim", "iosSimulatorArm64"),
            ("x86_64-apple-ios", "iosX64"),
        ];

        for (target, slice) in targets {
            let lib = self.cargo_build_staticlib(target, "ios")?;
            let dest = self
                .out_dir
                .join("kotlin/cinterop/ios")
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
