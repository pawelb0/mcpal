use std::collections::BTreeMap;
use std::path::Path;

use mcpal_core::ServerSpec;
use serde::Deserialize;
use serde_json::Value;

use crate::{DiscoveredServer, Scope};

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

/// Walk a nested key path against a JSON value, returning the final object map.
pub(crate) fn walk_key_path<'a>(
    root: &'a Value,
    path: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    let mut cur = root;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_object()
}

/// Parse the common `{ "<name>": { command|url, ... } }` map shape into a list
/// of `DiscoveredServer`. Entries that don't match either variant are dropped.
pub fn servers_map(
    obj: &serde_json::Map<String, Value>,
    source: &'static str,
    source_path: &Path,
    scope: Scope,
) -> Vec<DiscoveredServer> {
    obj.iter()
        .filter_map(|(name, val)| {
            let entry = Entry::deserialize(val).ok()?;
            Some(DiscoveredServer {
                source,
                source_path: source_path.to_path_buf(),
                name: name.clone(),
                spec: entry.into_spec(),
                scope,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn walks_single_level() {
        let v = json!({ "mcpServers": { "a": {} } });
        let m = walk_key_path(&v, &["mcpServers"]).unwrap();
        assert!(m.contains_key("a"));
    }

    #[test]
    fn walks_three_levels() {
        let v = json!({ "chat": { "mcp": { "servers": { "a": {} } } } });
        let m = walk_key_path(&v, &["chat", "mcp", "servers"]).unwrap();
        assert!(m.contains_key("a"));
    }

    #[test]
    fn missing_segment_returns_none() {
        let v = json!({ "chat": { } });
        assert!(walk_key_path(&v, &["chat", "mcp", "servers"]).is_none());
    }

    #[test]
    fn non_object_terminal_returns_none() {
        let v = json!({ "k": 7 });
        assert!(walk_key_path(&v, &["k"]).is_none());
    }
}
