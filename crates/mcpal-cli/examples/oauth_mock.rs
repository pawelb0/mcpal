//! Minimal OAuth 2.1 + RFC 9728 + DCR mock used by the integration harness.
//!
//! Endpoints:
//!   GET  /.well-known/oauth-protected-resource    → points at this same server
//!   GET  /.well-known/oauth-authorization-server  → AS metadata
//!   POST /register                                → DCR (RFC 7591)
//!   GET  /authorize                               → 302 to redirect_uri with code+state
//!   POST /token                                   → access/refresh token JSON
//!
//! Spawn:
//!   oauth_mock          (binds 127.0.0.1:0, prints `port=<n>` then runs)
//!   oauth_mock 9999     (binds 127.0.0.1:9999)
//!
//! Stops on SIGTERM/SIGINT.

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::{
    Form, Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{Value, json};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
        .await
        .expect("bind");
    let bound = listener.local_addr().expect("local_addr").port();
    let base = Arc::new(format!("http://127.0.0.1:{bound}"));
    let app = Router::new()
        .route("/.well-known/oauth-protected-resource", get(prm))
        .route("/.well-known/oauth-authorization-server", get(asm))
        .route("/register", post(register))
        .route("/authorize", get(authorize))
        .route("/token", post(token))
        .with_state(base);
    println!("port={bound}");
    axum::serve(listener, app).await.expect("serve");
}

async fn prm(State(base): State<Arc<String>>) -> Json<Value> {
    Json(json!({"resource": *base, "authorization_servers": [*base]}))
}

async fn asm(State(base): State<Arc<String>>) -> Json<Value> {
    Json(json!({
        "issuer": *base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "registration_endpoint": format!("{base}/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
    }))
}

async fn register(Json(body): Json<Value>) -> Json<Value> {
    let redirect_uris = body
        .get("redirect_uris")
        .cloned()
        .unwrap_or_else(|| json!([]));
    Json(json!({
        "client_id": "mock-client-id",
        "client_secret": null,
        "client_name": "mcpal-test",
        "redirect_uris": redirect_uris,
        "token_endpoint_auth_method": "none",
    }))
}

#[derive(Deserialize)]
struct AuthorizeQuery {
    redirect_uri: String,
    state: String,
}

async fn authorize(Query(q): Query<AuthorizeQuery>) -> impl IntoResponse {
    let sep = if q.redirect_uri.contains('?') { '&' } else { '?' };
    let url = format!("{}{sep}code=mock-code&state={}", q.redirect_uri, q.state);
    Redirect::to(&url)
}

async fn token(Form(form): Form<HashMap<String, String>>) -> impl IntoResponse {
    let grant = form.get("grant_type").map(String::as_str).unwrap_or("");
    if grant == "authorization_code" || grant == "refresh_token" {
        return (
            StatusCode::OK,
            Json(json!({
                "access_token": "mock-access",
                "token_type": "Bearer",
                "expires_in": 3600,
                "refresh_token": "mock-refresh",
            })),
        );
    }
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "unsupported_grant_type"})),
    )
}
