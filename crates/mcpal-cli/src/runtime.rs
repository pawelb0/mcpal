use std::cell::OnceCell;
use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use crate::output::{Format, emit_list, emit_one};
use anyhow::{Result, anyhow};
use mcpal_core::{AuthSpec, Client, Handler, ServerSpec, connect};
use mcpal_discovery::{DiscoveredServer, DiscoveryCtx, discover};
use serde::Serialize;

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
    pub handler: Handler,
    discovered: OnceCell<Vec<DiscoveredServer>>,
}

impl Ctx {
    pub fn new(
        cfg: Config,
        format: Format,
        query: Option<String>,
        timeout: Option<u64>,
        config_path: PathBuf,
        handler: Handler,
    ) -> Self {
        Self {
            cfg,
            format,
            query,
            timeout,
            config_path,
            handler,
            discovered: OnceCell::new(),
        }
    }

    /// Race a request future against `--timeout` and Ctrl-C. Returns the
    /// future's output verbatim. A fired timeout becomes an `anyhow!` whose
    /// message `exit::classify` matches as E0007; Ctrl-C becomes E0011.
    pub async fn under_deadline<F: Future>(&self, fut: F) -> Result<F::Output> {
        let timeout = self.timeout;
        let sleeper = async move {
            match timeout {
                Some(secs) => tokio::time::sleep(Duration::from_secs(secs)).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::pin!(fut, sleeper);
        tokio::select! {
            out = &mut fut => Ok(out),
            _ = &mut sleeper => Err(anyhow!(
                "request timed out after {}s",
                timeout.expect("sleeper only fires when timeout is set"),
            )),
            _ = tokio::signal::ctrl_c() => Err(anyhow!("interrupted by ctrl-c")),
        }
    }

    pub fn render_one<T: Serialize>(&self, value: &T) -> Result<(), crate::output::Error> {
        if let Some(q) = self.query.as_deref() {
            let v = serde_json::to_value(value)?;
            let filtered = crate::output::apply_query(v, Some(q))?;
            return emit_one(self.format, &filtered);
        }
        emit_one(self.format, value)
    }

    pub fn render_list<T: Serialize>(&self, items: &[T]) -> Result<(), crate::output::Error> {
        if let Some(q) = self.query.as_deref() {
            let v = serde_json::to_value(items)?;
            let filtered = crate::output::apply_query(v, Some(q))?;
            return emit_one(self.format, &filtered);
        }
        emit_list(self.format, items)
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
        attach_bearer(&mut resolved.spec, reference, &resolved.display).await;
        let client = connect(&resolved.spec, self.handler.clone()).await?;
        Ok((resolved, client))
    }
}

/// For HTTP specs: replace `AuthSpec::Oauth` with the stored access token
/// (auto-refresh if near expiry); leave any other explicit `auth` alone;
/// otherwise fall through oauth → keyring → `MCPAL_BEARER`.
async fn attach_bearer(spec: &mut ServerSpec, reference: &str, display: &str) {
    let ServerSpec::Http { url, auth, .. } = spec else {
        return;
    };
    if matches!(auth, Some(AuthSpec::Oauth)) {
        match oauth::access_token_refreshing(reference, url).await {
            Some(token) => *auth = Some(AuthSpec::Bearer { token }),
            None => eprintln!(
                "warning: '{display}' is configured for OAuth but no token is stored; \
                 run `mcpal auth login --oauth {display}`"
            ),
        }
        return;
    }
    if auth.is_some() {
        return;
    }
    let token = oauth::access_token_refreshing(reference, url)
        .await
        .or_else(|| keyring::get(reference, Kind::Bearer))
        .or_else(|| std::env::var("MCPAL_BEARER").ok());
    if let Some(token) = token {
        *auth = Some(AuthSpec::Bearer { token });
    }
}
