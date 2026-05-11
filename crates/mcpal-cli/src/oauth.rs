//! OAuth 2.1 login flow: loopback callback + browser launch + token persistence.

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

use crate::keyring;

const CALLBACK_HTML: &str = "<!doctype html><html><body><h2>mcpal: authorized.</h2>\
<p>You can close this tab.</p></body></html>";

pub struct KeyringCredentialStore {
    reference: String,
}

impl KeyringCredentialStore {
    pub fn new(reference: impl Into<String>) -> Self {
        Self {
            reference: reference.into(),
        }
    }
}

#[async_trait]
impl CredentialStore for KeyringCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let Some(json) = keyring::get_oauth_blob(&self.reference) else {
            return Ok(None);
        };
        serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| AuthError::InternalError(format!("decode creds: {e}")))
    }

    async fn save(&self, c: StoredCredentials) -> Result<(), AuthError> {
        let json = serde_json::to_string(&c)
            .map_err(|e| AuthError::InternalError(format!("encode creds: {e}")))?;
        keyring::put_oauth_blob(&self.reference, &json)
            .map_err(|e| AuthError::InternalError(format!("store creds: {e:#}")))
    }

    async fn clear(&self) -> Result<(), AuthError> {
        keyring::delete_oauth_blob(&self.reference)
            .map_err(|e| AuthError::InternalError(format!("delete creds: {e:#}")))
    }
}

#[derive(Debug, Deserialize)]
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
            let sender = sender.clone();
            move |Query(p): Query<CallbackParams>| async move {
                if let Some(s) = sender.lock().await.take() {
                    let _ = s.send(p);
                }
                Html(CALLBACK_HTML)
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .context("bind callback listener")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let mut am = AuthorizationManager::new(server_url)
        .await
        .context("init AuthorizationManager")?;
    am.set_credential_store(KeyringCredentialStore::new(reference));
    am.discover_metadata().await.context("discover metadata")?;
    am.register_client("mcpal", &redirect_uri, &[])
        .await
        .context("register client")?;

    let url = am
        .get_authorization_url(&[])
        .await
        .context("authorization url")?;

    eprintln!("Open this URL to authorize {reference}:");
    eprintln!("  {url}");
    if open_browser {
        let _ = webbrowser::open(&url);
    }

    let params = rx.await.context("waiting for callback")?;
    am.exchange_code_for_token(&params.code, &params.state)
        .await
        .context("exchange code for token")?;

    server.abort();
    Ok(())
}

pub async fn refresh(reference: &str, server_url: &str) -> Result<()> {
    let store = KeyringCredentialStore::new(reference);
    if store.load().await?.is_none() {
        bail!("no oauth credentials stored for '{reference}'; run `mcpal auth login --oauth`");
    }
    let mut am = AuthorizationManager::new(server_url)
        .await
        .context("init AuthorizationManager")?;
    am.set_credential_store(store);
    let restored = am
        .initialize_from_store()
        .await
        .context("restore creds from keyring")?;
    if !restored {
        bail!("credentials present but could not be restored; re-run login");
    }
    am.refresh_token().await.context("refresh token")?;
    Ok(())
}

/// Read the current access token from stored credentials. Walks serialized
/// JSON directly to skip an `oauth2` crate dep just for one trait method.
pub fn current_access_token(reference: &str) -> Option<String> {
    let json = keyring::get_oauth_blob(reference)?;
    let v: serde_json::Value = serde_json::from_str(&json).ok()?;
    v.pointer("/token_response/access_token")?
        .as_str()
        .map(String::from)
}
