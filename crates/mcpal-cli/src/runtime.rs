use std::path::PathBuf;

use anyhow::Result;
use mcpal_core::{Client, connect};
use mcpal_output::Format;
use serde_json::Value;

use crate::config::Config;
use crate::resolver::{ResolvedServer, resolve};

pub struct Ctx {
    pub cfg: Config,
    pub format: Format,
    pub config_path: PathBuf,
}

impl Ctx {
    pub async fn open(&self, reference: &str) -> Result<(ResolvedServer, Client)> {
        let resolved = resolve(reference, &self.cfg)?;
        let client = connect(&resolved.spec).await?;
        Ok((resolved, client))
    }
}

pub struct Probe {
    pub name: String,
    pub version: String,
    pub info: Option<Value>,
}

/// Summarize the server's `initialize` response. Server-info fields are
/// `serverInfo/{name,version}` per the MCP spec (camelCase).
pub fn probe(client: &Client) -> Probe {
    let info = client
        .peer_info()
        .and_then(|i| serde_json::to_value(i).ok());
    let pick = |k: &str| {
        info.as_ref()
            .and_then(|v| {
                v.pointer(&format!("/serverInfo/{k}"))
                    .and_then(Value::as_str)
            })
            .unwrap_or(if k == "name" { "unknown" } else { "?" })
            .to_string()
    };
    Probe {
        name: pick("name"),
        version: pick("version"),
        info,
    }
}
