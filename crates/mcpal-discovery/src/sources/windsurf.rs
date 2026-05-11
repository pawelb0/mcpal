use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{Ctx, DiscoveredServer, Scope, Source};

pub struct Windsurf;

impl Source for Windsurf {
    fn id(&self) -> &'static str {
        "windsurf"
    }
    fn display_name(&self) -> &'static str {
        "Windsurf"
    }

    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf> {
        vec![ctx.home.join(".codeium/windsurf/mcp_config.json")]
    }

    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let Some(map) = v.get("mcpServers").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, "windsurf", path, Scope::Global))
    }
}
