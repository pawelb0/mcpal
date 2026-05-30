//! OAuth 2.1 login: loopback callback + browser launch + token persistence.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use axum::{Router, extract::Query, response::Html, routing::get};
use mcpal_core::rmcp::transport::{
    AuthError, AuthorizationManager, CredentialStore, StoredCredentials,
};
use serde::Deserialize;
use tokio::sync::{Mutex, oneshot};

use crate::keyring::{self, Kind};

const CALLBACK_HTML: &str =
    "<!doctype html><html><body><h2>mcpal: authorized.</h2><p>Close this tab.</p></body></html>";

pub(crate) struct KeyringCredentialStore {
    reference: String,
}

impl KeyringCredentialStore {
    pub fn new(reference: &str) -> Self {
        Self {
            reference: reference.into(),
        }
    }
}

fn internal(ctx: &str, e: impl std::fmt::Display) -> AuthError {
    AuthError::InternalError(format!("{ctx}: {e}"))
}

#[async_trait]
impl CredentialStore for KeyringCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        match keyring::get(&self.reference, Kind::Oauth) {
            Some(j) => serde_json::from_str(&j)
                .map(Some)
                .map_err(|e| internal("decode", e)),
            None => Ok(None),
        }
    }
    async fn save(&self, c: StoredCredentials) -> Result<(), AuthError> {
        let j = serde_json::to_string(&c).map_err(|e| internal("encode", e))?;
        keyring::put(&self.reference, Kind::Oauth, &j).map_err(|e| internal("store", e))
    }
    async fn clear(&self) -> Result<(), AuthError> {
        keyring::delete(&self.reference, Kind::Oauth).map_err(|e| internal("delete", e))
    }
}

#[derive(Deserialize)]
struct CallbackParams {
    code: String,
    state: String,
}

pub async fn login(reference: &str, server_url: &str, open_browser: bool) -> Result<()> {
    let (tx, rx) = oneshot::channel::<CallbackParams>();
    let sender = Arc::new(Mutex::new(Some(tx)));
    let app = Router::new().route(
        "/callback",
        get({
            let s = sender.clone();
            move |Query(p): Query<CallbackParams>| async move {
                if let Some(tx) = s.lock().await.take() {
                    let _ = tx.send(p);
                }
                Html(CALLBACK_HTML)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let redirect_uri = format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr()?.port()
    );
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let mut am = AuthorizationManager::new(server_url).await?;
    am.set_credential_store(KeyringCredentialStore::new(reference));
    let md = am.discover_metadata().await?;
    am.set_metadata(md);
    am.register_client("mcpal", &redirect_uri, &[]).await?;
    let url = am.get_authorization_url(&[]).await?;

    eprintln!("Open this URL to authorize {reference}:\n  {url}");
    if open_browser && let Err(e) = webbrowser::open(&url) {
        eprintln!("  (couldn't launch browser: {e}; open the URL manually)");
    }

    let p = rx.await.context("waiting for oauth callback")?;
    am.exchange_code_for_token(&p.code, &p.state).await?;
    server.abort();
    Ok(())
}

pub async fn refresh(reference: &str, server_url: &str) -> Result<()> {
    let store = KeyringCredentialStore::new(reference);
    if store.load().await?.is_none() {
        bail!("no oauth credentials for '{reference}'; run `mcpal auth login --oauth`");
    }
    let mut am = AuthorizationManager::new(server_url).await?;
    am.set_credential_store(store);
    if !am.initialize_from_store().await? {
        bail!("credentials present but could not be restored; re-run login");
    }
    am.refresh_token().await?;
    Ok(())
}

fn token_blob(reference: &str) -> Option<serde_json::Value> {
    serde_json::from_str(&keyring::get(reference, Kind::Oauth)?).ok()
}

fn access_token_from(blob: &serde_json::Value) -> Option<String> {
    blob.pointer("/token_response/access_token")?
        .as_str()
        .map(String::from)
}

pub(crate) fn current_access_token(reference: &str) -> Option<String> {
    access_token_from(&token_blob(reference)?)
}

/// `current_access_token`, but eagerly refreshes when within 30s of expiry.
pub(crate) async fn access_token_refreshing(reference: &str, server_url: &str) -> Option<String> {
    let mut blob = token_blob(reference)?;
    if expires_soon(&blob) {
        if let Err(e) = refresh(reference, server_url).await {
            tracing::debug!(target: "mcpal::oauth", "eager refresh failed: {e:#}");
        }
        blob = token_blob(reference)?;
    }
    access_token_from(&blob)
}

fn expires_soon(v: &serde_json::Value) -> bool {
    let r = v.pointer("/token_received_at").and_then(|x| x.as_u64());
    let t = v
        .pointer("/token_response/expires_in")
        .and_then(|x| x.as_u64());
    let (Some(r), Some(t)) = (r, t) else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    now + 30 >= r + t
}
