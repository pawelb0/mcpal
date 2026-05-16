//! MCP Registry client (registry.modelcontextprotocol.io).

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use mcpal_core::ServerSpec;
use serde::Deserialize;

const DEFAULT_BASE: &str = "https://registry.modelcontextprotocol.io";

#[derive(Deserialize, Debug)]
pub struct Envelope {
    pub servers: Vec<ServerWrapper>,
}

#[derive(Deserialize, Debug)]
pub struct ServerWrapper {
    pub server: Server,
}

#[derive(Deserialize, Debug)]
pub struct Server {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub packages: Vec<Package>,
    #[serde(default)]
    pub remotes: Vec<Remote>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub registry_type: String,
    pub identifier: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub transport: Option<Transport>,
    #[serde(default)]
    pub environment_variables: Vec<EnvVar>,
    #[serde(default)]
    pub package_arguments: Vec<Argument>,
    #[serde(default)]
    pub runtime_arguments: Vec<Argument>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Transport {
    #[serde(default)]
    pub r#type: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    #[serde(default)]
    pub is_required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Argument {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Remote {
    pub r#type: String,
    pub url: String,
}

#[derive(serde::Serialize, Debug)]
pub struct Hit<'a> {
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    pub kind: &'static str,
}

pub fn classify(server: &Server) -> &'static str {
    if !server.packages.is_empty() {
        "stdio"
    } else if !server.remotes.is_empty() {
        "http"
    } else {
        "unknown"
    }
}

fn base_url() -> String {
    std::env::var("MCPAL_REGISTRY_URL").unwrap_or_else(|_| DEFAULT_BASE.into())
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("mcpal/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()?)
}

pub async fn search(query: &str, limit: u32) -> Result<Envelope> {
    let url = format!("{}/v0/servers", base_url());
    let resp = client()?
        .get(&url)
        .query(&[("search", query), ("limit", &limit.to_string())])
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

/// Fetch a single server by exact registry name. The registry's
/// `?name=` filter doesn't actually filter — we run a `?search=` and
/// keep only the entry whose `name` matches verbatim.
pub async fn fetch(name: &str) -> Result<Server> {
    let env = search(name, 20).await?;
    env.servers
        .into_iter()
        .map(|w| w.server)
        .find(|s| s.name == name)
        .ok_or_else(|| {
            anyhow!("registry: no exact match for '{name}' (try `mcpal server search {name}`)")
        })
}

/// Build a `ServerSpec` from a registry entry. `extra_env` provides
/// `--env K=V` overrides for required environment variables.
pub fn to_spec(server: &Server, extra_env: &BTreeMap<String, String>) -> Result<ServerSpec> {
    if let Some(pkg) = pick_stdio_package(server) {
        return stdio_from_package(pkg, extra_env);
    }
    if let Some(remote) = server
        .remotes
        .iter()
        .find(|r| r.r#type == "streamable-http")
    {
        return Ok(ServerSpec::Http {
            url: remote.url.clone(),
            headers: BTreeMap::new(),
            auth: None,
        });
    }
    bail!(
        "registry server '{}' has no stdio package or streamable-http remote",
        server.name
    )
}

fn pick_stdio_package(server: &Server) -> Option<&Package> {
    server.packages.iter().find(|p| {
        p.transport
            .as_ref()
            .map(|t| t.r#type == "stdio")
            .unwrap_or(true)
    })
}

fn stdio_from_package(pkg: &Package, extra_env: &BTreeMap<String, String>) -> Result<ServerSpec> {
    let (command, mut args) = match pkg.registry_type.as_str() {
        "npm" => (
            "npx".to_string(),
            vec!["-y".into(), npm_target(&pkg.identifier, pkg.version.as_deref())],
        ),
        "pypi" => ("uvx".into(), vec![pkg.identifier.clone()]),
        "oci" => (
            "docker".into(),
            vec!["run".into(), "--rm".into(), "-i".into(), pkg.identifier.clone()],
        ),
        other => bail!("unsupported registry_type '{other}' (try `mcpal raw` or file an issue)"),
    };
    args.extend(arg_values(&pkg.package_arguments));
    args.extend(arg_values(&pkg.runtime_arguments));

    let mut env: BTreeMap<String, String> = pkg
        .environment_variables
        .iter()
        .filter_map(|v| v.default.as_ref().map(|d| (v.name.clone(), d.clone())))
        .collect();
    for (k, v) in extra_env {
        env.insert(k.clone(), v.clone());
    }
    let missing: Vec<&str> = pkg
        .environment_variables
        .iter()
        .filter(|v| v.is_required && !env.contains_key(&v.name))
        .map(|v| v.name.as_str())
        .collect();
    if !missing.is_empty() {
        bail!(
            "registry server requires env vars; pass `--env {}=…`",
            missing.join("=… --env "),
        );
    }

    Ok(ServerSpec::Stdio { command, args, env })
}

fn npm_target(identifier: &str, version: Option<&str>) -> String {
    match version {
        Some(v) if !v.is_empty() => format!("{identifier}@{v}"),
        _ => identifier.to_string(),
    }
}

fn arg_values(args: &[Argument]) -> impl Iterator<Item = String> + '_ {
    args.iter()
        .filter_map(|a| a.value.clone().or_else(|| a.default.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_remote_envelope() {
        let body = r#"{
            "servers":[{"server":{
                "name":"x/y","version":"0.1",
                "remotes":[{"type":"streamable-http","url":"https://a.example/mcp"}]
            }}],
            "metadata":{"nextCursor":"next","count":1}
        }"#;
        let env: Envelope = serde_json::from_str(body).unwrap();
        assert_eq!(env.servers.len(), 1);
        assert_eq!(classify(&env.servers[0].server), "http");
        let spec = to_spec(&env.servers[0].server, &BTreeMap::new()).unwrap();
        assert!(matches!(spec, ServerSpec::Http { .. }));
    }

    #[test]
    fn parses_npm_package_and_builds_npx() {
        let body = r#"{"servers":[{"server":{
            "name":"x/y",
            "packages":[{
                "registryType":"npm",
                "identifier":"@mcp/foo",
                "version":"1.2.3",
                "transport":{"type":"stdio"},
                "environmentVariables":[{"name":"API_KEY","isRequired":true}]
            }]
        }}]}"#;
        let env: Envelope = serde_json::from_str(body).unwrap();
        let mut extra = BTreeMap::new();
        extra.insert("API_KEY".into(), "k".into());
        let spec = to_spec(&env.servers[0].server, &extra).unwrap();
        let ServerSpec::Stdio { command, args, env } = spec else {
            panic!("expected stdio")
        };
        assert_eq!(command, "npx");
        assert_eq!(args, vec!["-y", "@mcp/foo@1.2.3"]);
        assert_eq!(env.get("API_KEY"), Some(&"k".to_string()));
    }

    #[test]
    fn errors_when_required_env_missing() {
        let body = r#"{"servers":[{"server":{
            "name":"x/y",
            "packages":[{
                "registryType":"npm",
                "identifier":"@mcp/foo",
                "environmentVariables":[{"name":"NEEDED","isRequired":true}]
            }]
        }}]}"#;
        let env: Envelope = serde_json::from_str(body).unwrap();
        let err = to_spec(&env.servers[0].server, &BTreeMap::new()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("NEEDED"), "{msg}");
    }
}
