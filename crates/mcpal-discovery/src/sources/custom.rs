use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{DiscoveredServer, DiscoveryCtx, Scope, Source};

pub struct CustomFile;

impl Source for CustomFile {
    fn id(&self) -> &'static str {
        "custom"
    }

    fn paths(&self, ctx: &DiscoveryCtx) -> Vec<(PathBuf, Scope)> {
        ctx.custom_paths
            .iter()
            .map(|p| (p.clone(), Scope::Global))
            .collect()
    }

    fn parse(&self, path: &Path, scope: Scope, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let Some(map) = v.get("mcpServers").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, "custom", path, scope))
    }
}
