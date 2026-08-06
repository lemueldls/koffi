use facet::Facet;

/// The `[config]` root of the config file (see the `config` field). Read
/// from `config.toml` / `config.jsonc` / `config.json` next to where `koffi`
/// runs (or `--config <path>`), with CLI overrides on top.
#[derive(Facet, Debug)]
pub struct KoffiConfig {
    #[facet(default = vec![TargetPlatform::Jvm])]
    pub platforms: Vec<TargetPlatform>,

    #[facet(default = JvmBackend::Ffm)]
    pub jvm_backend: JvmBackend,

    #[facet(default = vec![AndroidAbi::Arm64V8a, AndroidAbi::X86_64])]
    pub android_abis: Vec<AndroidAbi>,
}

impl KoffiConfig {
    #[must_use]
    pub fn has(&self, platform: &TargetPlatform) -> bool {
        self.platforms.iter().any(|p| p == platform)
    }

    /// Every configured native platform, in config order.
    #[must_use]
    pub fn native_platforms(&self) -> Vec<&TargetPlatform> {
        self.platforms.iter().filter(|p| p.is_native()).collect()
    }

    /// `(abi, rust target triple)` pairs for every configured android abi.
    #[must_use]
    pub fn android_targets(&self) -> Vec<(&str, &str)> {
        self.android_abis
            .iter()
            .map(|abi| (abi.as_str(), abi.rust_target_triple()))
            .collect()
    }

    /// `(kotlin platform, rust target triple)` pairs for every configured
    /// native platform.
    #[must_use]
    pub fn native_targets(&self) -> Vec<(&str, &str)> {
        self.platforms
            .iter()
            .filter_map(|p| Some((p.as_str(), p.native_target_triple()?)))
            .collect()
    }

    /// True when the jvm actual object is the JNI backend.
    #[must_use]
    pub fn jvm_uses_jni(&self) -> bool {
        self.jvm_backend == JvmBackend::Jni
    }
}

#[repr(u8)]
#[derive(Facet, Debug, Clone, PartialEq, Eq)]
pub enum TargetPlatform {
    #[facet(rename = "jvm")]
    Jvm,
    #[facet(rename = "android")]
    Android,
    #[facet(rename = "linuxX64")]
    LinuxX64,
    #[facet(rename = "linuxArm64")]
    LinuxArm64,
    #[facet(rename = "macosX64")]
    MacosX64,
    #[facet(rename = "macosArm64")]
    MacosArm64,
    #[facet(rename = "mingwX64")]
    MingwX64,
}

impl TargetPlatform {
    #[must_use]
    pub fn all() -> &'static [TargetPlatform] {
        &[
            TargetPlatform::Jvm,
            TargetPlatform::Android,
            TargetPlatform::LinuxX64,
            TargetPlatform::LinuxArm64,
            TargetPlatform::MacosX64,
            TargetPlatform::MacosArm64,
            TargetPlatform::MingwX64,
        ]
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            TargetPlatform::Jvm => "jvm",
            TargetPlatform::Android => "android",
            TargetPlatform::LinuxX64 => "linuxX64",
            TargetPlatform::LinuxArm64 => "linuxArm64",
            TargetPlatform::MacosX64 => "macosX64",
            TargetPlatform::MacosArm64 => "macosArm64",
            TargetPlatform::MingwX64 => "mingwX64",
        }
    }

    #[must_use]
    pub fn is_native(&self) -> bool {
        !matches!(self, TargetPlatform::Jvm | TargetPlatform::Android)
    }

    #[must_use]
    pub fn native_target_triple(&self) -> Option<&str> {
        match self {
            TargetPlatform::LinuxX64 => Some("x86_64-unknown-linux-gnu"),
            TargetPlatform::LinuxArm64 => Some("aarch64-unknown-linux-gnu"),
            TargetPlatform::MacosX64 => Some("x86_64-apple-darwin"),
            TargetPlatform::MacosArm64 => Some("aarch64-apple-darwin"),
            TargetPlatform::MingwX64 => Some("x86_64-pc-windows-gnu"),
            _ => None,
        }
    }

    #[must_use]
    pub fn native_def_suffix(&self) -> Option<&str> {
        match self {
            TargetPlatform::LinuxX64 => Some("linux_x64"),
            TargetPlatform::LinuxArm64 => Some("linux_arm64"),
            TargetPlatform::MacosX64 => Some("macos_x64"),
            TargetPlatform::MacosArm64 => Some("macos_arm64"),
            TargetPlatform::MingwX64 => Some("mingw_x64"),
            _ => None,
        }
    }
}

/// Backend used for the `jvm` platform's actual object.
#[repr(u8)]
#[derive(Facet, Debug, Clone, Copy, PartialEq, Eq)]
#[facet(rename_all = "snake_case")]
pub enum JvmBackend {
    #[facet(default)]
    Ffm,
    Jni,
}

#[repr(u8)]
#[derive(Facet, Debug)]
pub enum AndroidAbi {
    #[facet(rename = "arm64-v8a")]
    Arm64V8a,
    #[facet(rename = "armeabi-v7a")]
    ArmeabiV7a,
    #[facet(rename = "x86_64")]
    X86_64,
    #[facet(rename = "x86")]
    X86,
}

impl AndroidAbi {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            AndroidAbi::Arm64V8a => "arm64-v8a",
            AndroidAbi::ArmeabiV7a => "armeabi-v7a",
            AndroidAbi::X86_64 => "x86_64",
            AndroidAbi::X86 => "x86",
        }
    }

    #[must_use]
    pub fn rust_target_triple(&self) -> &'static str {
        match self {
            AndroidAbi::Arm64V8a => "aarch64-linux-android",
            AndroidAbi::ArmeabiV7a => "armv7-linux-androideabi",
            AndroidAbi::X86 => "i686-linux-android",
            AndroidAbi::X86_64 => "x86_64-linux-android",
        }
    }
}
