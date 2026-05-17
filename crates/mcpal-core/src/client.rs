use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use tokio::process::Command;

use crate::handler::Handler;
use crate::{AuthSpec, Error, Result, ServerSpec};

pub type Client = RunningService<RoleClient, Handler>;

pub async fn connect(spec: &ServerSpec, handler: Handler) -> Result<Client> {
    match spec {
        ServerSpec::Stdio { command, args, env } => {
            connect_stdio(command, args, env, handler).await
        }
        ServerSpec::Http { url, headers, auth } => {
            connect_http(url, headers, auth.as_ref(), handler).await
        }
    }
}

async fn connect_stdio(
    command: &str,
    args: &[String],
    env: &std::collections::BTreeMap<String, String>,
    handler: Handler,
) -> Result<Client> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    // Drop child stderr by default so banner lines and any stray ANSI escape
    // sequences from the spawned server don't bleed into our terminal.
    // `MCPAL_CHILD_STDERR=inherit` opts back in for debugging.
    let inherit = std::env::var("MCPAL_CHILD_STDERR")
        .map(|v| v == "inherit")
        .unwrap_or(false);
    if !inherit {
        cmd.stderr(std::process::Stdio::null());
    }
    // Detach the child from our controlling terminal so uv/npx-style
    // installers can't write progress UI to /dev/tty over our alt-screen.
    #[cfg(unix)]
    detach_session(&mut cmd);
    let transport = TokioChildProcess::new(cmd)?;
    handler
        .serve(transport)
        .await
        .map_err(|e| Error::Service(e.to_string()))
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn detach_session(cmd: &mut Command) {
    // SAFETY: setsid is async-signal-safe and only mutates the child's
    // process group / session, not the parent's.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}

async fn connect_http(
    url: &str,
    headers: &std::collections::BTreeMap<String, String>,
    auth: Option<&AuthSpec>,
    handler: Handler,
) -> Result<Client> {
    let mut config = StreamableHttpClientTransportConfig::with_uri(url.to_string());
    if let Some(h) = resolve_auth(auth)? {
        config = config.auth_header(h);
    }
    if !headers.is_empty() {
        let mut map = HashMap::with_capacity(headers.len());
        for (k, v) in headers {
            let name = HeaderName::try_from(k.as_str())
                .map_err(|e| Error::Service(format!("bad header name {k}: {e}")))?;
            let value = HeaderValue::try_from(v.as_str())
                .map_err(|e| Error::Service(format!("bad header value for {k}: {e}")))?;
            map.insert(name, value);
        }
        config = config.custom_headers(map);
    }

    let transport = StreamableHttpClientTransport::from_config(config);
    handler
        .serve(transport)
        .await
        .map_err(|e| Error::Service(e.to_string()))
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn resolve_auth(auth: Option<&AuthSpec>) -> Result<Option<String>> {
    match auth {
        None => Ok(None),
        Some(AuthSpec::Bearer { token }) => Ok(Some(bearer(token))),
        Some(AuthSpec::BearerEnv { env }) => match std::env::var(env) {
            Ok(token) => Ok(Some(bearer(&token))),
            Err(_) => Err(Error::Auth(format!("env var {env} not set"))),
        },
        Some(AuthSpec::Oauth) => Err(Error::Unsupported("OAuth (M4)")),
    }
}
