use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{Ctx, DiscoveredServer, Scope, Source};

pub struct LmStudio;

impl Source for LmStudio {
    fn id(&self) -> &'static str {
        "lm-studio"
    }
    fn display_name(&self) -> &'static str {
        "LM Studio"
    }

    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf> {
        vec![ctx.home.join(".lmstudio/mcp.json")]
    }

    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let Some(map) = v.get("mcpServers").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, "lm-studio", path, Scope::Global))
    }
}
