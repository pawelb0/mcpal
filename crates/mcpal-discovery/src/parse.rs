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
            let entry: Entry = serde_json::from_value(val.clone()).ok()?;
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
