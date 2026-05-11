use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{Ctx, DiscoveredServer, Scope, Source};

pub struct ClaudeCode;

impl Source for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude-code"
    }
    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf> {
        let mut p = vec![ctx.home.join(".claude.json")];
        let project_mcp = ctx.cwd.join(".mcp.json");
        if project_mcp.exists() {
            p.push(project_mcp);
        }
        p
    }

    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let mut out = Vec::new();

        if let Some(map) = v.get("mcpServers").and_then(Value::as_object) {
            out.extend(servers_map(map, "claude-code", path, Scope::Global));
        }
        if let Some(projects) = v.get("projects").and_then(Value::as_object) {
            for proj in projects.values() {
                if let Some(map) = proj.get("mcpServers").and_then(Value::as_object) {
                    out.extend(servers_map(map, "claude-code", path, Scope::Project));
                }
            }
        }
        Ok(out)
    }
}
