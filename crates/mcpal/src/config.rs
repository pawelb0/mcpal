use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use mcpal_core::ServerSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub server: BTreeMap<String, ServerSpec>,
}

/// `$MCPAL_CONFIG` if set, otherwise the OS-appropriate config dir
/// (`~/Library/Application Support/mcpal` on macOS, `$XDG_CONFIG_HOME/mcpal`
/// on Linux), falling back to `./mcpal.toml`.
pub fn default_path() -> PathBuf {
    if let Ok(p) = std::env::var("MCPAL_CONFIG") {
        return PathBuf::from(p);
    }
    ProjectDirs::from("", "", "mcpal")
        .map(|d| d.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("mcpal.toml"))
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
        };
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
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

        let cfg = Config {
            server: BTreeMap::from([(
                "everything".into(),
                ServerSpec::Stdio {
                    command: "npx".into(),
                    args: vec![
                        "-y".into(),
                        "@modelcontextprotocol/server-everything".into(),
                    ],
                    env: BTreeMap::new(),
                },
            )]),
        };
        cfg.save(&path).unwrap();

        let back = Config::load(&path).unwrap();
        assert!(back.server.contains_key("everything"));
    }

    #[test]
    fn missing_file_is_empty_default() {
        let cfg = Config::load(Path::new("/no/such/file/mcpal.toml")).unwrap();
        assert!(cfg.server.is_empty());
    }
}
