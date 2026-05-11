use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{Ctx, DiscoveredServer, Scope, Source};

pub struct Cursor;

impl Source for Cursor {
    fn id(&self) -> &'static str {
        "cursor"
    }
    fn display_name(&self) -> &'static str {
        "Cursor"
    }

    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf> {
        let mut p = vec![ctx.home.join(".cursor/mcp.json")];
        let project = ctx.cwd.join(".cursor/mcp.json");
        if project.exists() {
            p.push(project);
        }
        p
    }

    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let Some(map) = v.get("mcpServers").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, "cursor", path, scope_of(path)))
    }
}

fn scope_of(path: &Path) -> Scope {
    let home = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_default();
    if path.starts_with(home.join(".cursor")) {
        Scope::Global
    } else {
        Scope::Project
    }
}
