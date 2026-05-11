use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{DiscoveredServer, DiscoveryCtx, Scope, Source};

pub struct ClaudeCode;

const ID: &str = "claude-code";

impl Source for ClaudeCode {
    fn id(&self) -> &'static str {
        ID
    }

    fn paths(&self, ctx: &DiscoveryCtx) -> Vec<(PathBuf, Scope)> {
        // `~/.claude.json` carries both user-level (Global) and per-project
        // (Project) entries inside; the in-file scope wins over the hint.
        vec![
            (ctx.home.join(".claude.json"), Scope::Global),
            (ctx.cwd.join(".mcp.json"), Scope::Project),
        ]
    }

    fn parse(&self, path: &Path, scope: Scope, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let mut out = Vec::new();

        if let Some(map) = v.get("mcpServers").and_then(Value::as_object) {
            out.extend(servers_map(map, ID, path, scope));
        }
        if let Some(projects) = v.get("projects").and_then(Value::as_object) {
            for proj in projects.values() {
                if let Some(map) = proj.get("mcpServers").and_then(Value::as_object) {
                    out.extend(servers_map(map, ID, path, Scope::Project));
                }
            }
        }
        Ok(out)
    }
}
