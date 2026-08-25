use std::{collections::BTreeMap, path::Path};

/// The source crate's build profile, resolved so the two cargo trees koffi
/// builds (the extraction build with the source crate as root, the glue
/// build with it as a dependency) compile it with identical flags. Cargo
/// gives a dependency the root manifest's profile plus config-file
/// overrides - `[profile.dev.package."*"]` and friends - which is exactly
/// how the two trees drift apart (say a root at `opt-level = 1` and a
/// dependency at 3). Pinning the same values on both sides keeps the two
/// compiles of a changed source crate on the same profile, and the
/// unchanged case fully warm; the crate itself still rebuilds once per
/// tree, because cargo's artifact metadata hash includes the workspace
/// root, so neither tree can reuse the other's artifacts.
///
/// Values come from the source crate's own `[profile.dev]`/`[profile.release]`
/// (merged with its workspace root manifest, member wins) overlaid on
/// cargo's built-in defaults. Config-file profiles are deliberately not
/// read: they are the drift source.
pub struct SourceProfile {
    pub dev: BTreeMap<String, toml::Value>,
    pub release: BTreeMap<String, toml::Value>,
}

/// The fingerprint-relevant keys cargo lets a `[profile.*.package.<name>]`
/// section pin, plus cargo's built-in defaults per profile.
/// `split-debuginfo` is handled separately: its built-in default is
/// platform-dependent, so it is only pinned when the manifest sets it.
const PROFILE_KEYS: [&str; 6] = [
    "opt-level",
    "debug",
    "codegen-units",
    "incremental",
    "overflow-checks",
    "strip",
];

/// `lto` and `panic` can't be pinned per-package (cargo rejects them: a
/// build must link and unwind uniformly), so they are fixed constants,
/// passed as root-level `--config` to both trees. The release values mirror
/// the glue manifest's own `[profile.release]`.
const GLOBAL_KEYS: [(&str, &str, &str); 2] =
    [("lto", "false", "thin"), ("panic", "unwind", "unwind")];

/// The typed value for a global key: `lto` is a boolean, `panic` a string.
fn global_value(key: &str, repr: &str) -> toml::Value {
    match key {
        "lto" => toml::Value::Boolean(repr == "true"),
        _ => toml::Value::String(repr.to_string()),
    }
}

fn default_profile(release: bool) -> BTreeMap<String, toml::Value> {
    let mut pins = if release {
        BTreeMap::from([
            ("opt-level".to_string(), toml::Value::Integer(3)),
            ("debug".to_string(), toml::Value::Integer(0)),
            ("codegen-units".to_string(), toml::Value::Integer(16)),
            ("incremental".to_string(), toml::Value::Boolean(false)),
            ("overflow-checks".to_string(), toml::Value::Boolean(false)),
            ("strip".to_string(), toml::Value::String("none".to_string())),
        ])
    } else {
        BTreeMap::from([
            ("opt-level".to_string(), toml::Value::Integer(0)),
            ("debug".to_string(), toml::Value::Integer(2)),
            ("codegen-units".to_string(), toml::Value::Integer(256)),
            ("incremental".to_string(), toml::Value::Boolean(true)),
            ("overflow-checks".to_string(), toml::Value::Boolean(true)),
            ("strip".to_string(), toml::Value::String("none".to_string())),
        ])
    };
    for (key, dev_repr, release_repr) in GLOBAL_KEYS {
        let repr = if release { release_repr } else { dev_repr };
        pins.insert(key.to_string(), global_value(key, repr));
    }
    pins
}

/// Reads `[profile.dev]`/`[profile.release]` from `crate_dir/Cargo.toml`.
/// A workspace-member crate gets its settings merged with the workspace
/// root manifest (member wins, per key), matching what cargo itself does
/// for the extraction build.
pub fn read_source_profile(crate_dir: &Path) -> anyhow::Result<SourceProfile> {
    let mut dev = default_profile(false);
    let mut release = default_profile(true);

    for manifest_path in source_manifests(crate_dir)? {
        let contents = std::fs::read_to_string(&manifest_path)?;
        let parsed: toml::Table = toml::from_str(&contents)?;
        for (section, pins) in [("dev", &mut dev), ("release", &mut release)] {
            let Some(table) = parsed.get("profile").and_then(|p| p.get(section)) else {
                continue;
            };
            let Some(table) = table.as_table() else {
                continue;
            };
            for (key, value) in table {
                if PROFILE_KEYS.contains(&key.as_str()) || key == "split-debuginfo" {
                    pins.insert(key.clone(), value.clone());
                }
            }
        }
    }

    Ok(SourceProfile { dev, release })
}

/// The manifests to read profile settings from, workspace root first (so
/// the merge overwrites it with member settings). A standalone crate is
/// its own root.
fn source_manifests(crate_dir: &Path) -> anyhow::Result<Vec<camino::Utf8PathBuf>> {
    let manifest_path = crate_dir.join("Cargo.toml");
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec()
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to read cargo metadata for `{}`: {e}",
                crate_dir.display()
            )
        })?;

    let mut manifests = vec![metadata.workspace_root.join("Cargo.toml")];
    if metadata.workspace_root != crate_dir {
        manifests.push(
            camino::Utf8PathBuf::from_path_buf(manifest_path).map_err(|p| {
                anyhow::anyhow!("source crate path is not valid UTF-8: {}", p.display())
            })?,
        );
    }
    Ok(manifests)
}

impl SourceProfile {
    /// `--config` values pinning the extraction build's root profile to
    /// the same values the glue tree resolves for the source crate:
    /// `profile.dev.opt-level=0`, `profile.release.lto="thin"`, ...
    pub fn config_args(&self) -> Vec<String> {
        let mut args = Vec::with_capacity(self.dev.len() + self.release.len());
        for (section, pins) in [("dev", &self.dev), ("release", &self.release)] {
            for (key, value) in pins {
                args.push(format!("profile.{section}.{key}={value}"));
            }
        }
        args
    }

    /// The root-level pins the glue builds need too: the per-package keys
    /// already live in the generated manifest, but `lto`/`panic` can only
    /// be pinned at the root, so the glue tree must be told them the same
    /// way the extraction build is.
    pub fn glue_config_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for (section, pins) in [("dev", &self.dev), ("release", &self.release)] {
            for (key, ..) in GLOBAL_KEYS {
                args.push(format!("profile.{section}.{key}={}", pins[key]));
            }
        }
        args
    }

    /// `key = value` lines for the glue manifest's
    /// `[profile.dev.package.<crate>]` pin section.
    pub fn dev_pin_lines(&self) -> Vec<String> {
        self.dev
            .iter()
            .filter(|(key, _)| !GLOBAL_KEYS.iter().any(|(k, ..)| k == *key))
            .map(|(k, v)| format!("{k} = {v}"))
            .collect()
    }

    /// Same for `[profile.release.package.<crate>]`.
    pub fn release_pin_lines(&self) -> Vec<String> {
        self.release
            .iter()
            .filter(|(key, _)| !GLOBAL_KEYS.iter().any(|(k, ..)| k == *key))
            .map(|(k, v)| format!("{k} = {v}"))
            .collect()
    }
}
