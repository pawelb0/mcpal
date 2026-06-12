use std::cell::OnceCell;
use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use crate::output::{Format, emit_one};
use anyhow::Result;
use mcpal_core::{AuthSpec, Client, Handler, ServerSpec, connect};
use mcpal_discovery::{DiscoveredServer, DiscoveryCtx, discover};
use serde::Serialize;

use crate::config::Config;
use crate::exit::CliError;
use crate::keyring::{self, Kind};
use crate::oauth;
use crate::resolver::{ResolvedServer, resolve};

pub struct Ctx {
    pub cfg: Config,
    pub format: Format,
    pub query: Option<String>,
    pub timeout: Option<u64>,
    pub config_path: PathBuf,
    pub collection_override: Option<PathBuf>,
    pub profile: String,
    pub discover_from: Vec<PathBuf>,
    pub handler: Handler,
    pub auth_override: Option<String>,
    discovered: OnceCell<Vec<DiscoveredServer>>,
}

impl Ctx {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cfg: Config,
        format: Format,
        query: Option<String>,
        timeout: Option<u64>,
        config_path: PathBuf,
        collection_override: Option<PathBuf>,
        profile: String,
        discover_from: Vec<PathBuf>,
        handler: Handler,
        auth_override: Option<String>,
    ) -> Self {
        Self {
            cfg,
            format,
            query,
            timeout,
            config_path,
            collection_override,
            profile,
            discover_from,
            handler,
            auth_override,
            discovered: OnceCell::new(),
        }
    }

    /// Race a request future against `--timeout` and Ctrl-C. Returns the
    /// future's output verbatim. A fired timeout becomes an `anyhow!` whose
    /// message `exit::classify` matches as E0007; Ctrl-C becomes E0011.
    pub async fn under_deadline<F: Future>(&self, fut: F) -> Result<F::Output> {
        race_deadline(self.timeout, fut).await
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
        self.render_one(&items)
    }

    pub fn discovered(&self) -> Result<&[DiscoveredServer]> {
        if self.discovered.get().is_none() {
            let dctx = DiscoveryCtx::current()?.with_custom_paths(self.discover_from.clone());
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

/// Map `--auth <mode>` to a concrete `AuthSpec` override (or `Ok(None)` for
/// "anonymous"). Returns `Err` for an unrecognised mode.
pub(crate) fn parse_auth_override(mode: &str) -> Result<Option<AuthSpec>> {
    match mode {
        "none" | "anon" => Ok(None),
        "oauth" => Ok(Some(AuthSpec::Oauth)),
        s if s.starts_with("env:") => Ok(Some(AuthSpec::BearerEnv {
            env: s["env:".len()..].into(),
        })),
        s if s.starts_with("bearer:") => Ok(Some(AuthSpec::Bearer {
            token: s["bearer:".len()..].into(),
        })),
        other => Err(CliError::Usage(format!(
            "--auth: unknown mode '{other}' (expected: oauth, none, env:VAR, bearer:TOKEN)"
        ))
        .into()),
    }
}

async fn race_deadline<F: Future>(timeout: Option<u64>, fut: F) -> Result<F::Output> {
    let sleeper = async move {
        match timeout {
            Some(secs) => tokio::time::sleep(Duration::from_secs(secs)).await,
            None => std::future::pending::<()>().await,
        }
    };
    tokio::pin!(fut, sleeper);
    tokio::select! {
        out = &mut fut => Ok(out),
        _ = &mut sleeper => Err(CliError::Timeout(
            timeout.expect("sleeper only fires when timeout is set"),
        )
        .into()),
        _ = tokio::signal::ctrl_c() => Err(CliError::Interrupted.into()),
    }
}

/// For HTTP specs: replace `AuthSpec::Oauth` with the stored access token
/// (auto-refresh if near expiry); leave any other explicit `auth` alone;
/// otherwise fall through oauth → keyring → `MCPAL_BEARER`.
pub async fn attach_bearer(spec: &mut ServerSpec, reference: &str, display: &str) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_timeout_returns_future_value() {
        let r: Result<i32> = race_deadline(None, async { 42 }).await;
        assert_eq!(r.unwrap(), 42);
    }

    #[tokio::test]
    async fn future_completing_before_timeout_returns_value() {
        let r: Result<&str> = race_deadline(Some(60), async { "ok" }).await;
        assert_eq!(r.unwrap(), "ok");
    }

    #[tokio::test(start_paused = true)]
    async fn elapsed_timeout_returns_e0007_message() {
        let r: Result<()> = race_deadline(Some(3), async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        })
        .await;
        let err = r.unwrap_err().to_string();
        assert!(err.contains("timed out"), "got: {err}");
        assert!(err.contains("3s"), "should report budget: {err}");
    }

    #[tokio::test(start_paused = true)]
    async fn fast_future_wins_against_long_timeout() {
        let r: Result<u32> = race_deadline(Some(3600), async {
            tokio::time::sleep(Duration::from_millis(1)).await;
            7
        })
        .await;
        assert_eq!(r.unwrap(), 7);
    }

    #[test]
    fn auth_override_oauth_explicit() {
        let a = parse_auth_override("oauth").unwrap();
        assert!(matches!(a, Some(AuthSpec::Oauth)));
    }

    #[test]
    fn auth_override_none_alias() {
        assert!(parse_auth_override("none").unwrap().is_none());
        assert!(parse_auth_override("anon").unwrap().is_none());
    }

    #[test]
    fn auth_override_env_var() {
        let a = parse_auth_override("env:GH_TOKEN").unwrap();
        assert!(matches!(a, Some(AuthSpec::BearerEnv { env }) if env == "GH_TOKEN"));
    }

    #[test]
    fn auth_override_bearer_literal() {
        let a = parse_auth_override("bearer:abc.def").unwrap();
        assert!(matches!(a, Some(AuthSpec::Bearer { token }) if token == "abc.def"));
    }

    #[test]
    fn auth_override_unknown_mode_errors() {
        let err = parse_auth_override("magic").unwrap_err().to_string();
        assert!(err.contains("unknown mode"));
        assert!(err.contains("oauth, none, env:VAR, bearer:TOKEN"));
    }
}
