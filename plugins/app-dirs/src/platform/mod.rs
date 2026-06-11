//! Platform dispatch for `app_dirs` and `android_init`.
//!
//! Each platform is a separate submodule gated by `#[cfg(...)]`. A
//! compile-time fallback is provided for unknown targets so that library
//! consumers do not get linker errors on exotic platforms.

use crate::AppDirs;

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod desktop;

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_family = "wasm")]
mod wasm;

/// Returns platform-appropriate `AppDirs` for `app_name`.
pub fn get(app_name: &str) -> AppDirs {
    get_impl(app_name)
}

/// Stores Android-provided paths. No-op on every other platform.
pub const fn android_init(
    files_dir: &str,
    cache_dir: &str,
    no_backup_dir: &str,
    external_files_dir: &str,
) {
    android_init_impl(files_dir, cache_dir, no_backup_dir, external_files_dir);
}

#[cfg(target_os = "android")]
fn get_impl(app_name: &str) -> AppDirs {
    android::get(app_name)
}

#[cfg(target_os = "android")]
fn android_init_impl(
    files_dir: &str,
    cache_dir: &str,
    no_backup_dir: &str,
    external_files_dir: &str,
) {
    android::init(files_dir, cache_dir, no_backup_dir, external_files_dir);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn get_impl(app_name: &str) -> AppDirs {
    desktop::get(app_name)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[allow(unused_variables)]
const fn android_init_impl(
    files_dir: &str,
    cache_dir: &str,
    no_backup_dir: &str,
    external_files_dir: &str,
) {
    // no-op
}

#[cfg(target_os = "ios")]
fn get_impl(app_name: &str) -> AppDirs {
    ios::get(app_name)
}

#[cfg(target_os = "ios")]
#[allow(unused_variables)]
fn android_init_impl(
    files_dir: &str,
    cache_dir: &str,
    no_backup_dir: &str,
    external_files_dir: &str,
) {
    // no-op
}

#[cfg(target_family = "wasm")]
fn get_impl(app_name: &str) -> AppDirs {
    wasm::get(app_name)
}

#[cfg(target_family = "wasm")]
#[allow(unused_variables)]
fn android_init_impl(
    files_dir: &str,
    cache_dir: &str,
    no_backup_dir: &str,
    external_files_dir: &str,
) {
    // no-op
}

#[cfg(not(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
    target_os = "ios",
    target_family = "wasm",
)))]
fn get_impl(app_name: &str) -> AppDirs {
    let base = format!("/tmp/{app_name}");
    AppDirs {
        data: format!("{base}/data"),
        config: format!("{base}/config"),
        cache: format!("{base}/cache"),
        temp: std::env::temp_dir().to_string_lossy().into_owned(),
    }
}

#[cfg(not(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
    target_os = "ios",
    target_family = "wasm",
)))]
#[allow(unused_variables)]
fn android_init_impl(
    files_dir: &str,
    cache_dir: &str,
    no_backup_dir: &str,
    external_files_dir: &str,
) {
    // no-op
}
