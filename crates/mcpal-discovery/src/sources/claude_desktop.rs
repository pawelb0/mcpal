use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{Ctx, DiscoveredServer, Scope, Source};

pub struct ClaudeDesktop;

impl Source for ClaudeDesktop {
    fn id(&self) -> &'static str {
        "claude-desktop"
    }
    fn display_name(&self) -> &'static str {
        "Claude Desktop"
    }

    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf> {
        // macOS only for now; Windows/Linux paths land later.
        vec![
            ctx.home
                .join("Library/Application Support/Claude/claude_desktop_config.json"),
        ]
    }

    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let Some(map) = v.get("mcpServers").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, "claude-desktop", path, Scope::Global))
    }
}
