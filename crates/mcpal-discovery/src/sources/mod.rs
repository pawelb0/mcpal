use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{DiscoveredServer, DiscoveryCtx, Scope, Source};

mod claude_code;
mod opencode;

pub use claude_code::ClaudeCode;
pub use opencode::Opencode;

/// Declarative spec for clients whose mcp config is a single JSON(C) file with
/// a `{ "<key>": { "<name>": entry, ... } }` map. Most clients fit this shape.
pub struct SimpleSource {
    pub id: &'static str,
    pub key: &'static str,
    pub global: &'static [&'static str],
    pub project: &'static [&'static str],
    pub jsonc: bool,
}

const SIMPLE_SOURCES: &[SimpleSource] = &[
    SimpleSource {
        id: "claude-desktop",
        key: "mcpServers",
        global: &["Library/Application Support/Claude/claude_desktop_config.json"],
        project: &[],
        jsonc: false,
    },
    SimpleSource {
        id: "cursor",
        key: "mcpServers",
        global: &[".cursor/mcp.json"],
        project: &[".cursor/mcp.json"],
        jsonc: false,
    },
    SimpleSource {
        id: "lm-studio",
        key: "mcpServers",
        global: &[".lmstudio/mcp.json"],
        project: &[],
        jsonc: false,
    },
    SimpleSource {
        id: "windsurf",
        key: "mcpServers",
        global: &[".codeium/windsurf/mcp_config.json"],
        project: &[],
        jsonc: false,
    },
    SimpleSource {
        id: "cline",
        key: "mcpServers",
        global: &["Library/Application Support/Code/User/globalStorage/\
             saoudrizwan.claude-dev/settings/cline_mcp_settings.json"],
        project: &[],
        jsonc: false,
    },
    SimpleSource {
        id: "zed",
        key: "context_servers",
        global: &[".config/zed/settings.json"],
        project: &[],
        jsonc: true,
    },
];

impl Source for &'static SimpleSource {
    fn id(&self) -> &'static str {
        self.id
    }
    fn paths(&self, ctx: &DiscoveryCtx) -> Vec<(PathBuf, Scope)> {
        let mut out = Vec::with_capacity(self.global.len() + self.project.len());
        out.extend(
            self.global
                .iter()
                .map(|p| (ctx.home.join(p), Scope::Global)),
        );
        out.extend(
            self.project
                .iter()
                .map(|p| (ctx.cwd.join(p), Scope::Project)),
        );
        out
    }
    fn parse(&self, path: &Path, scope: Scope, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = if self.jsonc {
            json5::from_str(std::str::from_utf8(bytes)?)?
        } else {
            serde_json::from_slice(bytes)?
        };
        let Some(map) = v.get(self.key).and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, self.id, path, scope))
    }
}

pub fn registry() -> Vec<Box<dyn Source>> {
    let mut v: Vec<Box<dyn Source>> = vec![Box::new(ClaudeCode), Box::new(Opencode)];
    for s in SIMPLE_SOURCES {
        v.push(Box::new(s));
    }
    v
}

/// Look up a registered source by id; used by tests and the discover command.
pub fn by_id(id: &str) -> Option<Box<dyn Source>> {
    registry().into_iter().find(|s| s.id() == id)
}
