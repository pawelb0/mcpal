use std::cell::OnceCell;
use std::path::PathBuf;

use anyhow::Result;
use mcpal_core::{AuthSpec, Client, Handler, ServerSpec, connect};
use mcpal_discovery::{DiscoveredServer, DiscoveryCtx, discover};
use mcpal_output::Format;
use serde_json::Value;

use crate::config::Config;
use crate::keyring::{self, Kind};
use crate::oauth;
use crate::resolver::{ResolvedServer, resolve};

pub struct Ctx {
    pub cfg: Config,
    pub format: Format,
    pub config_path: PathBuf,
    pub roots: Vec<String>,
    discovered: OnceCell<Vec<DiscoveredServer>>,
}

impl Ctx {
    pub fn new(cfg: Config, format: Format, config_path: PathBuf, roots: Vec<String>) -> Self {
        Self {
            cfg,
            format,
            config_path,
            roots,
            discovered: OnceCell::new(),
        }
    }

    pub fn discovered(&self) -> Result<&[DiscoveredServer]> {
        if self.discovered.get().is_none() {
            let dctx = DiscoveryCtx::current()?;
            let _ = self.discovered.set(discover(&dctx));
        }
        Ok(self.discovered.get().expect("just initialized").as_slice())
    }

    pub async fn open(&self, reference: &str) -> Result<(ResolvedServer, Client)> {
        let mut resolved = resolve(reference, self)?;
        attach_bearer(&mut resolved.spec, reference);
        let handler = Handler::default().with_roots(self.roots.clone());
        let client = connect(&resolved.spec, handler).await?;
        Ok((resolved, client))
    }
}

/// Resolve credentials for an HTTP spec. Skips stdio. If `auth` is already
/// `AuthSpec::Oauth` we replace it with the stored access token (or warn if
/// missing); any other explicit `auth` is left alone. With no explicit auth,
/// fall through: oauth blob → keyring bearer → `MCPAL_BEARER` env.
fn attach_bearer(spec: &mut ServerSpec, reference: &str) {
    let ServerSpec::Http { auth, .. } = spec else {
        return;
    };
    if let Some(AuthSpec::Oauth) = auth {
        match oauth::current_access_token(reference) {
            Some(token) => *auth = Some(AuthSpec::Bearer { token }),
            None => eprintln!(
                "warning: '{reference}' is configured for OAuth but no token is stored; \
                 run `mcpal auth login --oauth {reference}`"
            ),
        }
        return;
    }
    if auth.is_some() {
        return;
    }
    if let Some(token) = oauth::current_access_token(reference) {
        *auth = Some(AuthSpec::Bearer { token });
        return;
    }
    if let Some(token) = keyring::get(reference, Kind::Bearer) {
        *auth = Some(AuthSpec::Bearer { token });
        return;
    }
    if let Ok(token) = std::env::var("MCPAL_BEARER") {
        *auth = Some(AuthSpec::Bearer { token });
    }
}

pub struct Probe {
    pub name: String,
    pub version: String,
    pub info: Option<Value>,
}

/// MCP `serverInfo` uses camelCase — JSON pointers must match.
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
