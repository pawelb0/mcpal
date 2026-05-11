use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{Ctx, DiscoveredServer, Scope, Source};

pub struct Zed;

impl Source for Zed {
    fn id(&self) -> &'static str {
        "zed"
    }
    fn display_name(&self) -> &'static str {
        "Zed"
    }

    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf> {
        vec![ctx.home.join(".config/zed/settings.json")]
    }

    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let text = std::str::from_utf8(bytes)?;
        // Zed's settings.json is JSONC; json5 is a strict superset that handles it.
        let v: Value = json5::from_str(text)?;
        let Some(map) = v.get("context_servers").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, "zed", path, Scope::Global))
    }
}
