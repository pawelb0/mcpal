use std::cell::OnceCell;
use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, anyhow};
use mcpal_core::{AuthSpec, Client, Handler, HandlerOptions, ServerSpec, connect};
use mcpal_discovery::{DiscoveredServer, DiscoveryCtx, discover};
use mcpal_output::{Format, emit_list, emit_one};
use serde::Serialize;
use serde_json::Value;

use crate::config::Config;
use crate::keyring::{self, Kind};
use crate::oauth;
use crate::resolver::{ResolvedServer, resolve};

pub struct Ctx {
    pub cfg: Config,
    pub format: Format,
    pub query: Option<String>,
    pub timeout: Option<u64>,
    pub config_path: PathBuf,
    pub handler_opts: HandlerOptions,
    discovered: OnceCell<Vec<DiscoveredServer>>,
}

impl Ctx {
    pub fn new(
        cfg: Config,
        format: Format,
        query: Option<String>,
        timeout: Option<u64>,
        config_path: PathBuf,
        handler_opts: HandlerOptions,
    ) -> Self {
        Self {
            cfg,
            format,
            query,
            timeout,
            config_path,
            handler_opts,
            discovered: OnceCell::new(),
        }
    }

    /// Race a request future against `--timeout` and Ctrl-C. Returns the
    /// future's output verbatim; cancellation paths become anyhow errors that
    /// `exit::classify` maps to E0007 (timeout) and E0011 (interrupt).
    pub async fn under_deadline<F: Future>(&self, fut: F) -> Result<F::Output> {
        let sleeper: Box<dyn Future<Output = ()> + Unpin + Send> = match self.timeout {
            Some(secs) => Box::new(Box::pin(tokio::time::sleep(Duration::from_secs(secs)))),
            None => Box::new(Box::pin(std::future::pending::<()>())),
        };
        tokio::pin!(fut);
        tokio::select! {
            out = &mut fut => Ok(out),
            _ = sleeper => Err(anyhow!(
                "request timed out after {}s",
                self.timeout.unwrap_or_default()
            )),
            _ = tokio::signal::ctrl_c() => Err(anyhow!("interrupted by ctrl-c")),
        }
    }

    pub fn render_one<T: Serialize>(&self, value: &T) -> Result<(), mcpal_output::Error> {
        if let Some(q) = self.query.as_deref() {
            let v = serde_json::to_value(value)?;
            let filtered = mcpal_output::apply_query(v, Some(q))?;
            return emit_one(self.format, &filtered);
        }
        emit_one(self.format, value)
    }

    pub fn render_list<T: Serialize>(&self, items: &[T]) -> Result<(), mcpal_output::Error> {
        if let Some(q) = self.query.as_deref() {
            let v = serde_json::to_value(items)?;
            let filtered = mcpal_output::apply_query(v, Some(q))?;
            return emit_one(self.format, &filtered);
        }
        emit_list(self.format, items, &[], |_| Vec::new())
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
        let handler = Handler::new(self.handler_opts.clone());
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
