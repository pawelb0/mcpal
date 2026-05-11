use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt, service::RunningService};
use tokio::process::Command;

use crate::{AuthSpec, Error, Result, ServerSpec};

pub type Client = RunningService<RoleClient, ()>;

pub async fn connect(spec: &ServerSpec) -> Result<Client> {
    match spec {
        ServerSpec::Stdio { command, args, env } => connect_stdio(command, args, env).await,
        ServerSpec::Http { url, headers, auth } => connect_http(url, headers, auth.as_ref()).await,
    }
}

async fn connect_stdio(
    command: &str,
    args: &[String],
    env: &std::collections::BTreeMap<String, String>,
) -> Result<Client> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let transport = TokioChildProcess::new(cmd)?;
    ().serve(transport)
        .await
        .map_err(|e| Error::Service(e.to_string()))
}

async fn connect_http(
    url: &str,
    headers: &std::collections::BTreeMap<String, String>,
    auth: Option<&AuthSpec>,
) -> Result<Client> {
    let auth_header = resolve_auth(auth)?;

    let mut config = StreamableHttpClientTransportConfig::with_uri(url.to_string());
    if let Some(h) = auth_header {
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
    ().serve(transport)
        .await
        .map_err(|e| Error::Service(e.to_string()))
}

fn resolve_auth(auth: Option<&AuthSpec>) -> Result<Option<String>> {
    let Some(auth) = auth else {
        return Ok(std::env::var("MCPAL_BEARER")
            .ok()
            .map(|t| format!("Bearer {t}")));
    };
    match auth {
        AuthSpec::Bearer { token } => Ok(Some(format!("Bearer {token}"))),
        AuthSpec::BearerEnv { env } => match std::env::var(env) {
            Ok(token) => Ok(Some(format!("Bearer {token}"))),
            Err(_) => Err(Error::Auth(format!("env var {env} not set"))),
        },
        AuthSpec::Oauth => Err(Error::Unsupported("OAuth (M4)")),
    }
}
