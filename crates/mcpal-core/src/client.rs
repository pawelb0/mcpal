use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
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
    use std::collections::VecDeque;
    use std::process::Stdio;
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut cmd = Command::new(command);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    // Detach the child from our controlling terminal so uv/npx-style
    // installers can't write progress UI to /dev/tty over our alt-screen.
    #[cfg(unix)]
    detach_session(&mut cmd);

    // Three modes (env var `MCPAL_CHILD_STDERR`):
    //   `inherit` — pipe straight to parent's stderr (best for diagnosis).
    //   `null`    — discard. Set by the TUI to keep its alt-screen clean.
    //   default   — pipe + capture into a 64-line tail; flushed into the
    //               error chain on connect failure.
    let stderr_mode = std::env::var("MCPAL_CHILD_STDERR").unwrap_or_default();
    let (stderr_stdio, want_capture) = match stderr_mode.as_str() {
        "inherit" => (Stdio::inherit(), false),
        "null" => (Stdio::null(), false),
        _ => (Stdio::piped(), true),
    };

    let (transport, child_stderr) = rmcp::transport::TokioChildProcess::builder(cmd)
        .stderr(stderr_stdio)
        .spawn()?;

    let tail: Option<Arc<Mutex<VecDeque<String>>>> = if want_capture {
        let buf = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(64)));
        if let Some(err) = child_stderr {
            let buf2 = Arc::clone(&buf);
            tokio::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut q = buf2.lock().expect("stderr buffer mutex poisoned");
                    if q.len() == 64 {
                        q.pop_front();
                    }
                    q.push_back(line);
                }
            });
        }
        Some(buf)
    } else {
        None
    };

    match handler.serve(transport).await {
        Ok(client) => Ok(client),
        Err(e) => {
            let mut msg = e.to_string();
            if let Some(buf) = tail {
                // Give the drain task a tick to catch up.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let q = buf.lock().expect("stderr buffer mutex poisoned");
                if !q.is_empty() {
                    let lines: Vec<&str> = q.iter().map(String::as_str).collect();
                    msg = format!("{msg} (child stderr: {})", lines.join(" | "));
                }
            }
            Err(Error::Service(msg))
        }
    }
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
