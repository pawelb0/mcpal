use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use mcpal_core::ServerSpec;
use serde::Deserialize;
use serde_json::Value;

use crate::{Ctx, DiscoveredServer, Scope, Source};

pub struct Opencode;

impl Source for Opencode {
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn display_name(&self) -> &'static str {
        "opencode"
    }

    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf> {
        let mut p = vec![
            ctx.home.join(".config/opencode/opencode.json"),
            ctx.home.join(".config/opencode/opencode.jsonc"),
        ];
        for name in ["opencode.json", "opencode.jsonc"] {
            let project = ctx.cwd.join(name);
            if project.exists() {
                p.push(project);
            }
        }
        p
    }

    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let text = std::str::from_utf8(bytes)?;
        let v: Value = json5::from_str(text)?;
        let Some(map) = v.get("mcp").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };

        let scope = if path.starts_with(home_dot_config()) {
            Scope::Global
        } else {
            Scope::Project
        };

        Ok(map
            .iter()
            .filter_map(|(name, entry)| {
                let parsed: OpencodeEntry = serde_json::from_value(entry.clone()).ok()?;
                Some(DiscoveredServer {
                    source: "opencode",
                    source_path: path.to_path_buf(),
                    name: name.clone(),
                    spec: parsed.into_spec()?,
                    scope,
                })
            })
            .collect())
    }
}

fn home_dot_config() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().join(".config"))
        .unwrap_or_default()
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum OpencodeEntry {
    Local {
        command: Vec<String>,
        #[serde(default)]
        environment: BTreeMap<String, String>,
    },
    Remote {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl OpencodeEntry {
    fn into_spec(self) -> Option<ServerSpec> {
        Some(match self {
            Self::Local {
                command,
                environment,
            } => {
                let mut iter = command.into_iter();
                let cmd = iter.next()?;
                ServerSpec::Stdio {
                    command: cmd,
                    args: iter.collect(),
                    env: environment,
                }
            }
            Self::Remote { url, headers } => ServerSpec::Http {
                url,
                headers,
                auth: None,
            },
        })
    }
}
