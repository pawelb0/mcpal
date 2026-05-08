use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mcpal_core::ServerSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_profile_name")]
    pub default_profile: String,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profile: BTreeMap<String, Profile>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub server: BTreeMap<String, ServerSpec>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_server: Option<String>,
}

fn default_profile_name() -> String {
    "default".into()
}

/// Default config location. Honors `MCPAL_CONFIG`, then `XDG_CONFIG_HOME`,
/// then `$HOME/.config/mcpal/config.toml`.
pub fn default_path() -> PathBuf {
    if let Ok(p) = std::env::var("MCPAL_CONFIG") {
        return PathBuf::from(p);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("mcpal").join("config.toml");
    }
    if let Some(base) = directories::BaseDirs::new() {
        return base.home_dir().join(".config").join("mcpal").join("config.toml");
    }
    PathBuf::from("mcpal.toml")
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                default_profile: default_profile_name(),
                ..Self::default()
            });
        }
        let text =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("mkdir {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serialize config")?;
        fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut cfg = Config::default();
        cfg.default_profile = "default".into();
        cfg.server.insert(
            "everything".into(),
            ServerSpec::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "@modelcontextprotocol/server-everything".into()],
                env: BTreeMap::new(),
            },
        );
        cfg.save(&path).unwrap();

        let back = Config::load(&path).unwrap();
        assert_eq!(back.default_profile, "default");
        assert!(back.server.contains_key("everything"));
    }

    #[test]
    fn missing_file_is_empty_default() {
        let cfg = Config::load(Path::new("/no/such/file/mcpal.toml")).unwrap();
        assert_eq!(cfg.default_profile, "default");
        assert!(cfg.server.is_empty());
    }
}
