use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{Ctx, DiscoveredServer, Scope, Source};

pub struct Cline;

impl Source for Cline {
    fn id(&self) -> &'static str {
        "cline"
    }
    fn display_name(&self) -> &'static str {
        "Cline"
    }

    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf> {
        vec![ctx.home.join(
            "Library/Application Support/Code/User/globalStorage/\
             saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
        )]
    }

    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let Some(map) = v.get("mcpServers").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, "cline", path, Scope::Global))
    }
}
