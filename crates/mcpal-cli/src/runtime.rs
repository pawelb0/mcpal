use std::cell::OnceCell;
use std::path::PathBuf;

use anyhow::Result;
use mcpal_core::{AuthSpec, Client, ServerSpec, connect};
use mcpal_discovery::{DiscoveredServer, DiscoveryCtx, discover};
use mcpal_output::Format;
use serde_json::Value;

use crate::config::Config;
use crate::keyring;
use crate::resolver::{ResolvedServer, resolve};

pub struct Ctx {
    pub cfg: Config,
    pub format: Format,
    pub config_path: PathBuf,
    discovered: OnceCell<Vec<DiscoveredServer>>,
}

impl Ctx {
    pub fn new(cfg: Config, format: Format, config_path: PathBuf) -> Self {
        Self {
            cfg,
            format,
            config_path,
            discovered: OnceCell::new(),
        }
    }

    /// Run discovery on first access; subsequent calls reuse the result.
    pub fn discovered(&self) -> Result<&[DiscoveredServer]> {
        if self.discovered.get().is_none() {
            let dctx = DiscoveryCtx::current()?;
            let _ = self.discovered.set(discover(&dctx));
        }
        Ok(self.discovered.get().expect("just initialized").as_slice())
    }

    pub async fn open(&self, reference: &str) -> Result<(ResolvedServer, Client)> {
        let mut resolved = resolve(reference, self)?;
        inject_keyring_bearer(&mut resolved.spec, reference);
        let client = connect(&resolved.spec).await?;
        Ok((resolved, client))
    }
}

/// Patch an HTTP spec with a keyring-stored bearer when no explicit auth is set.
fn inject_keyring_bearer(spec: &mut ServerSpec, reference: &str) {
    let ServerSpec::Http { auth, .. } = spec else {
        return;
    };
    if auth.is_some() {
        return;
    }
    if let Some(token) = keyring::get_bearer(reference) {
        *auth = Some(AuthSpec::Bearer { token });
    }
}

pub struct Probe {
    pub name: String,
    pub version: String,
    pub info: Option<Value>,
}

/// Server name + version from the `initialize` response. MCP uses camelCase
/// for the serverInfo object — JSON pointers must match.
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
