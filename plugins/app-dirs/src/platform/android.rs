//! Android platform implementation.
//!
//! Android provides application-specific directories only through a Java
//! `android.content.Context` object. Because crossing the JNI boundary with
//! a `Context` reference is fragile (GC roots, threading rules), koffi instead
//! passes pre-extracted path strings from Kotlin to Rust once at startup via
//! [`init`]. The extracted paths are cached in a `OnceLock` for the lifetime
//! of the process.
//!
//! If [`init`] has not been called before [`get`], a best-effort fallback based
//! on the well-known `/data/data/<package>` convention is returned. This
//! fallback is correct for standard non-split APKs; adoptable-storage and
//! split-APK configurations may deviate.

use std::sync::OnceLock;

use crate::AppDirs;

struct AndroidState {
    /// `context.filesDir.absolutePath`
    files_dir: String,
    /// `context.cacheDir.absolutePath`
    cache_dir: String,
    /// `context.noBackupFilesDir.absolutePath`
    no_backup_dir: String,
    /// `context.getExternalFilesDir(null)?.absolutePath ?: ""`
    external_files_dir: String,
}

static STATE: OnceLock<AndroidState> = OnceLock::new();

/// Store paths extracted from an Android `Context`.
///
/// Subsequent calls after the first are silently ignored (`OnceLock`
/// semantics).
pub fn init(files_dir: &str, cache_dir: &str, no_backup_dir: &str, external_files_dir: &str) {
    // OnceLock::set returns Err if already set ignore gracefully.
    let _ = STATE.set(AndroidState {
        files_dir: files_dir.to_owned(),
        cache_dir: cache_dir.to_owned(),
        no_backup_dir: no_backup_dir.to_owned(),
        external_files_dir: external_files_dir.to_owned(),
    });
}

/// Returns app directories, using the stored `Context`-derived paths when
/// available or falling back to standard Android paths otherwise.
pub fn get(app_name: &str) -> AppDirs {
    match STATE.get() {
        Some(s) => from_state(s),
        None => fallback(app_name),
    }
}

fn from_state(s: &AndroidState) -> AppDirs {
    // Prefer no-backup dir for config/data (survives ADB backup by default).
    // Use files_dir as a last resort for the data field when no_backup_dir is
    // indistinguishable (same path on many devices).
    let data = if s.no_backup_dir.is_empty() {
        s.files_dir.clone()
    } else {
        s.no_backup_dir.clone()
    };

    AppDirs {
        data: s.files_dir.clone(),
        config: data,
        cache: s.cache_dir.clone(),
        temp: android_temp(),
    }
}

/// Best-effort fallback paths when `init()` was not called.
///
/// On a standard Android device, the app's user-data partition is always at
/// `/data/data/<package-name>/...`. However, koffi-plugin-app-dirs does not
/// know the package name at Rust compile-time, so we use `app_name` as a
/// placeholder. If `app_name` equals the actual package name this will be
/// correct; otherwise call `init()` from Kotlin.
fn fallback(app_name: &str) -> AppDirs {
    let base = format!("/data/data/{app_name}");

    AppDirs {
        data: format!("{base}/files"),
        config: format!("{base}/files"),
        cache: format!("{base}/cache"),
        temp: android_temp(),
    }
}

/// Returns the system temp directory on Android.
///
/// `/data/local/tmp` is the standard writable temp location for app processes.
/// `std::env::temp_dir()` returns the same on Android via Bionic's `getenv`.
fn android_temp() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}
