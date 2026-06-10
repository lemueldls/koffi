//! WASM (browser) platform stub.
//!
//! Browser environments have no real filesystem. This module returns virtual
//! path strings that are suitable for use as **key prefixes** in browser
//! storage APIs (IndexedDB, localStorage, Origin Private File System).
//!
//! The paths follow a `/app/{app_name}/…` convention so that they are:
//! - Hierarchical and namespaced per application.
//! - Clearly virtual (start with `/app/` rather than a real FS root).
//! - Safe to use as OPFS directory names via the Origin Private File System API.
//!
//! # What to do with these paths on the Kotlin/WASM side
//!
//! Use them as keys in `window.localStorage`, `IDBDatabase` object store
//! names, or as OPFS directory paths via `navigator.storage.getDirectory()`.
//! They are NOT valid arguments to `open(2)` or any POSIX syscall.

use crate::AppDirs;

pub fn get(app_name: &str) -> AppDirs {
    AppDirs {
        data: format!("/app/{app_name}/data"),
        config: format!("/app/{app_name}/config"),
        cache: format!("/app/{app_name}/cache"),
        // OPFS spec guarantees a writable temporary space at this path in some
        // implementations; for others treat it as a logical namespace.
        temp: format!("/app/{app_name}/tmp"),
    }
}
