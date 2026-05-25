use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{DiscoveredServer, DiscoveryCtx, Location, Scope, Source};

mod claude_code;
mod opencode;

pub use claude_code::ClaudeCode;
pub use opencode::Opencode;

pub enum SourceFormat {
    Json,
    Jsonc,
    Toml,
}

/// Declarative spec for clients whose mcp config is a single file with
/// a nested key path leading to a `{ "<name>": entry, ... }` map.
pub struct SimpleSource {
    pub id: &'static str,
    pub key_path: &'static [&'static str],
    pub global: &'static [(Location, &'static str)],
    pub project: &'static [&'static str],
    pub format: SourceFormat,
}

const SIMPLE_SOURCES: &[SimpleSource] = &[
    SimpleSource {
        id: "claude-desktop",
        key_path: &["mcpServers"],
        global: &[(Location::Config, "Claude/claude_desktop_config.json")],
        project: &[],
        format: SourceFormat::Json,
    },
    SimpleSource {
        id: "cursor",
        key_path: &["mcpServers"],
        global: &[(Location::Home, ".cursor/mcp.json")],
        project: &[".cursor/mcp.json"],
        format: SourceFormat::Json,
    },
    SimpleSource {
        id: "lm-studio",
        key_path: &["mcpServers"],
        global: &[(Location::Home, ".lmstudio/mcp.json")],
        project: &[],
        format: SourceFormat::Json,
    },
    SimpleSource {
        id: "windsurf",
        key_path: &["mcpServers"],
        global: &[(Location::Home, ".codeium/windsurf/mcp_config.json")],
        project: &[],
        format: SourceFormat::Json,
    },
    SimpleSource {
        id: "cline",
        key_path: &["mcpServers"],
        // VS Code globalStorage lives under Application Support / .config / AppData per platform.
        global: &[(
            Location::Config,
            "Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
        )],
        project: &[],
        format: SourceFormat::Json,
    },
    SimpleSource {
        id: "zed",
        key_path: &["context_servers"],
        global: &[(Location::Home, ".config/zed/settings.json")],
        project: &[],
        format: SourceFormat::Jsonc,
    },
    SimpleSource {
        id: "vscode",
        key_path: &["servers"],
        global: &[(Location::Config, "Code/User/mcp.json")],
        project: &[".vscode/mcp.json"],
        format: SourceFormat::Jsonc,
    },
    SimpleSource {
        id: "vscode-user",
        key_path: &["chat", "mcp", "servers"],
        global: &[(Location::Config, "Code/User/settings.json")],
        project: &[],
        format: SourceFormat::Jsonc,
    },
    SimpleSource {
        id: "continue",
        key_path: &["mcpServers"],
        global: &[(
            Location::Config,
            "Code/User/globalStorage/continue.continue/config.json",
        )],
        project: &[],
        format: SourceFormat::Json,
    },
    SimpleSource {
        id: "codex",
        key_path: &["mcp_servers"],
        global: &[(Location::Home, ".codex/config.toml")],
        project: &[],
        format: SourceFormat::Toml,
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
                .map(|(loc, rel)| (ctx.root_for(*loc).join(rel), Scope::Global)),
        );
        out.extend(
            self.project
                .iter()
                .map(|p| (ctx.cwd.join(p), Scope::Project)),
        );
        out
    }
    fn parse(&self, path: &Path, scope: Scope, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = match self.format {
            SourceFormat::Json => serde_json::from_slice(bytes)?,
            SourceFormat::Jsonc => json5::from_str(std::str::from_utf8(bytes)?)?,
            SourceFormat::Toml => {
                let s = std::str::from_utf8(bytes)?;
                let t: toml::Value = toml::from_str(s)?;
                serde_json::to_value(t)?
            }
        };
        let Some(map) = crate::parse::walk_key_path(&v, self.key_path) else {
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

pub fn by_id(id: &str) -> Option<Box<dyn Source>> {
    registry().into_iter().find(|s| s.id() == id)
}
