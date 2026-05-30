//! Read a Claude Desktop / Cursor / VS Code-style `mcp.json` and produce
//! a `BTreeMap<String, ServerSpec>` that overlays into mcpal config.
//!
//! Accepts a top-level `{"mcpServers": { "<name>": {...}, ... }}` object
//! or just the inner `{ "<name>": {...} }` map. Each entry is either an
//! stdio spec (`{command, args?, env?}`) or an HTTP spec (`{url, headers?}`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use mcpal_core::ServerSpec;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Stdio {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Http {
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Entry {
    Stdio(Stdio),
    Http(Http),
}

impl Entry {
    fn into_spec(self) -> ServerSpec {
        match self {
            Self::Stdio(s) => ServerSpec::Stdio {
                command: s.command,
                args: s.args,
                env: s.env,
            },
            Self::Http(h) => ServerSpec::Http {
                url: h.url,
                headers: h.headers,
                auth: None,
            },
        }
    }
}

pub fn load(path: &Path) -> Result<BTreeMap<String, ServerSpec>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let v: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    let map = v
        .get("mcpServers")
        .and_then(Value::as_object)
        .or_else(|| v.as_object())
        .ok_or_else(|| {
            anyhow!(
                "{} has neither a top-level mcpServers object nor a top-level server map",
                path.display()
            )
        })?;

    let mut out = BTreeMap::new();
    for (name, raw) in map {
        let entry = Entry::deserialize(raw)
            .with_context(|| format!("entry '{name}' in {}", path.display()))?;
        out.insert(name.clone(), entry.into_spec());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<BTreeMap<String, ServerSpec>> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        std::fs::write(&path, s).unwrap();
        load(&path)
    }

    #[test]
    fn cursor_shape() {
        let m = parse(r#"{"mcpServers":{"linear":{"url":"https://mcp.linear.app/sse"}}}"#).unwrap();
        assert!(matches!(m.get("linear"), Some(ServerSpec::Http { .. })));
    }

    #[test]
    fn claude_desktop_shape() {
        let m = parse(
            r#"{"mcpServers":{"fs":{"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem","/tmp"]}}}"#,
        )
        .unwrap();
        assert!(matches!(m.get("fs"), Some(ServerSpec::Stdio { .. })));
    }

    #[test]
    fn flat_map_also_accepted() {
        let m = parse(r#"{"foo":{"command":"echo"}}"#).unwrap();
        assert_eq!(m.len(), 1);
    }
}
