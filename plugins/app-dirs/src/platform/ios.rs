//! iOS platform implementation.
//!
//! On iOS every app runs inside a sandbox container. The OS assigns a unique
//! UUID-based container path at install time. The correct way to obtain the
//! container root is `NSHomeDirectory()`, a plain C function exported by the
//! Foundation framework. No Objective-C runtime machinery required.
//!
//! Standard subdirectory conventions inside the container:
//!
//! | Purpose | Path |
//! |---|---|
//! | Persistent data (backed up by iCloud) | `<home>/Documents/` |
//! | App support data (backed up, not user-visible) | `<home>/Library/Application Support/` |
//! | Caches (NOT backed up, may be purged) | `<home>/Library/Caches/` |
//! | Temporary (NOT backed up, purged on reboot) | result of `NSTemporaryDirectory()` |
//!
//! We map koffi's `data` and `config` to `Application Support` (the
//! conventional location for opaque app state), `cache` to `Library/Caches`,
//! and `temp` to `NSTemporaryDirectory()`.
//!
//! # Linking
//!
//! `NSHomeDirectory` and `NSTemporaryDirectory` are part of the Foundation
//! framework. The Xcode / Kotlin/Native build system links Foundation
//! automatically for iOS targets so no explicit `-framework Foundation` flag
//! is needed in the `.def` file beyond what cinterop adds by default.

use std::ffi::{CStr, c_char};

use crate::AppDirs;

extern "C" {
    /// Returns the home directory for the current user (or process sandbox on iOS).
    ///
    /// The returned pointer is valid for the lifetime of the process and must
    /// NOT be freed by the caller. It is effectively `'static` from Rust's
    /// perspective.
    fn NSHomeDirectory() -> *const c_char;

    /// Returns a string that is the path to a temporary directory.
    ///
    /// Same lifetime rules as `NSHomeDirectory`.
    fn NSTemporaryDirectory() -> *const c_char;
}

pub fn get(_app_name: &str) -> AppDirs {
    // `app_name` is unused on iOS because the OS already isolates each app
    // in its own container; there is no shared directory to partition.
    let home = ns_home_dir();
    let temp = ns_temp_dir();

    AppDirs {
        data: format!("{home}/Library/Application Support"),
        config: format!("{home}/Library/Application Support"),
        cache: format!("{home}/Library/Caches"),
        temp,
    }
}

/// Calls `NSHomeDirectory()` and converts the result to an owned `String`.
///
/// Falls back to the `HOME` environment variable, then to a hard-coded
/// sandbox-style path if neither is available (should not happen in practice).
fn ns_home_dir() -> String {
    // SAFETY: NSHomeDirectory returns a valid, non-null, null-terminated C
    // string owned by Foundation with process-lifetime validity.
    unsafe {
        let ptr = NSHomeDirectory();
        if ptr.is_null() {
            return std::env::var("HOME").unwrap_or_else(|_| "/var/mobile".to_owned());
        }

        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

/// Calls `NSTemporaryDirectory()` and converts the result to an owned `String`.
///
/// The returned path includes a trailing `/` on iOS, which we strip for
/// consistency with other platforms.
fn ns_temp_dir() -> String {
    // SAFETY: NSTemporaryDirectory returns a valid, non-null, null-terminated
    // C string owned by Foundation with process-lifetime validity.
    unsafe {
        let ptr = NSTemporaryDirectory();
        if ptr.is_null() {
            return std::env::temp_dir().to_string_lossy().into_owned();
        }

        let s = CStr::from_ptr(ptr).to_string_lossy();

        s.trim_end_matches('/').to_owned()
    }
}
