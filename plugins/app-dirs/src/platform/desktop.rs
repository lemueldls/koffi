//! Desktop platform implementation: Linux, macOS, Windows.
//!
//! Uses the [`dirs`] crate which follows:
//! - **Linux** - XDG Base Directory Specification
//! - **macOS** - `~/Library/...` conventions
//! - **Windows** - `%APPDATA%` / `%LOCALAPPDATA%` `SHGetKnownFolderPath` paths
//!
//! Each path has `app_name` appended so directories are isolated per application.

use crate::AppDirs;

pub fn get(app_name: &str) -> AppDirs {
    AppDirs {
        data: resolve(dirs::data_dir(), app_name, "data"),
        config: resolve(dirs::config_dir(), app_name, "config"),
        cache: resolve(dirs::cache_dir(), app_name, "cache"),
        temp: std::env::temp_dir()
            .join(app_name)
            .to_string_lossy()
            .into_owned(),
    }
}

/// Appends `app_name` to the directory returned by `dirs`, falling back to
/// `~/.{kind}/{app_name}` or `/tmp/{app_name}/{kind}` if unavailable.
fn resolve(base: Option<std::path::PathBuf>, app_name: &str, kind: &str) -> String {
    base.map(|p| p.join(app_name))
        .or_else(|| {
            dirs::home_dir().map(|h| {
                // Hidden dot-directory under $HOME as secondary fallback.
                h.join(format!(".{app_name}")).join(kind)
            })
        })
        .unwrap_or_else(|| std::path::PathBuf::from(format!("/tmp/{app_name}/{kind}")))
        .to_string_lossy()
        .into_owned()
}
