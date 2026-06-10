//! Platform-specific application data directory resolution for Koffi.
//!
//! # Usage
//!
//! ```rust
//! let dirs = koffi_plugin_app_dirs::app_dirs("my-app");
//! println!("data:   {}", dirs.data);
//! println!("cache:  {}", dirs.cache);
//! println!("config: {}", dirs.config);
//! println!("temp:   {}", dirs.temp);
//! ```
//!
//! # Android
//!
//! On Android the OS provides directories through the `Application` `Context`.
//! You must call the generated `AppDirsAndroidInit.init(context)` Kotlin helper
//! once from `Application.onCreate()` before calling `appDirs()` on the Kotlin
//! side. If `appDirsAndroidInit` has not been called the Rust side falls back
//! to derived `/data/data/<app>` paths, which are correct for non-split APKs
//! but may be wrong for some device configurations.
//!
//! # iOS
//!
//! On iOS the sandbox container directories are read via `NSHomeDirectory()`
//! and `NSTemporaryDirectory()` from Foundation without any additional
//! initialisation.
//!
//! # Desktop (Linux / macOS / Windows)
//!
//! Uses the `dirs` crate which follows XDG on Linux, `~/Library/...` on macOS,
//! and `%APPDATA%` / `%LOCALAPPDATA%` on Windows.

#![deny(unsafe_op_in_unsafe_fn)]

mod platform;

/// Platform-appropriate directories for an application to store its data.
///
/// All paths are absolute strings. The directories are not created
/// automatically; callers are responsible for calling `fs::create_dir_all`
/// before writing.
#[koffi::data]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppDirs {
    /// Primary persistent data directory. Never cleared automatically by the
    /// operating system.
    ///
    /// | Platform | Example |
    /// |---|---|
    /// | Android | `/data/data/com.example.app/files` |
    /// | iOS | `<container>/Library/Application Support` |
    /// | Linux | `~/.local/share/<app>` |
    /// | macOS | `~/Library/Application Support/<app>` |
    /// | Windows | `%APPDATA%\<app>` |
    pub data: String,

    /// User configuration directory.
    ///
    /// | Platform | Example |
    /// |---|---|
    /// | Android | same as `data` (no_backup_files_dir) |
    /// | iOS | same as `data` |
    /// | Linux | `~/.config/<app>` |
    /// | macOS | same as `data` |
    /// | Windows | same as `data` |
    pub config: String,

    /// Writable cache directory. May be cleared by the OS under storage
    /// pressure without notice.
    ///
    /// | Platform | Example |
    /// |---|---|
    /// | Android | `/data/data/com.example.app/cache` |
    /// | iOS | `<container>/Library/Caches` |
    /// | Linux | `~/.cache/<app>` |
    /// | macOS | `~/Library/Caches/<app>` |
    /// | Windows | `%LOCALAPPDATA%\<app>\cache` |
    pub cache: String,

    /// Temporary directory. Cleared on reboot or by the OS at any time.
    ///
    /// | Platform | Example |
    /// |---|---|
    /// | Android | `/data/local/tmp` |
    /// | iOS | `<container>/tmp/` |
    /// | Linux / macOS | `/tmp` |
    /// | Windows | `%TEMP%` |
    pub temp: String,
}

/// Return platform-appropriate application directories for `app_name`.
///
/// `app_name` is used as a path component on desktop platforms. It should be
/// a short identifier without path separators (e.g. `"my-app"` not
/// `"com.example/my-app"`).
///
/// On Android you must call [`app_dirs_android_init`] at least once before
/// invoking this function to obtain correct directories.
#[koffi::export]
#[must_use]
pub fn app_dirs(app_name: &str) -> AppDirs {
    platform::get(app_name)
}

/// Initialise app directory paths from the Android `Context`.
///
/// On all non-Android platforms this function is a no-op.
///
/// The generated Kotlin helper `AppDirsAndroidInit.init(context: Context)`
/// calls this automatically with the values extracted from the
/// `android.content.Context` object. You should not need to call this
/// function directly from Kotlin.
///
/// # Arguments
///
/// * `files_dir` - `context.filesDir.absolutePath`
/// * `cache_dir` - `context.cacheDir.absolutePath`
/// * `no_backup_dir` - `context.noBackupFilesDir.absolutePath`
/// * `external_files_dir` - `context.getExternalFilesDir(null)?.absolutePath ?: ""`
#[koffi::export]
pub fn app_dirs_android_init(
    files_dir: &str,
    cache_dir: &str,
    no_backup_dir: &str,
    external_files_dir: &str,
) {
    platform::android_init(files_dir, cache_dir, no_backup_dir, external_files_dir);
}
