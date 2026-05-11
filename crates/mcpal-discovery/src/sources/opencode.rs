use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use mcpal_core::ServerSpec;
use serde::Deserialize;
use serde_json::Value;

use crate::{DiscoveredServer, DiscoveryCtx, Scope, Source};

pub struct Opencode;

const ID: &str = "opencode";

impl Source for Opencode {
    fn id(&self) -> &'static str {
        ID
    }

    fn paths(&self, ctx: &DiscoveryCtx) -> Vec<(PathBuf, Scope)> {
        vec![
            (
                ctx.home.join(".config/opencode/opencode.json"),
                Scope::Global,
            ),
            (
                ctx.home.join(".config/opencode/opencode.jsonc"),
                Scope::Global,
            ),
            (ctx.cwd.join("opencode.json"), Scope::Project),
            (ctx.cwd.join("opencode.jsonc"), Scope::Project),
        ]
    }

    fn parse(&self, path: &Path, scope: Scope, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = json5::from_str(std::str::from_utf8(bytes)?)?;
        let Some(map) = v.get("mcp").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(map
            .iter()
            .filter_map(|(name, entry)| {
                let parsed: OpencodeEntry = serde_json::from_value(entry.clone()).ok()?;
                Some(DiscoveredServer {
                    source: ID,
                    source_path: path.to_path_buf(),
                    name: name.clone(),
                    spec: parsed.into_spec()?,
                    scope,
                })
            })
            .collect())
    }
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
