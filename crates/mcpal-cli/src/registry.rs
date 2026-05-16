//! MCP Registry client (registry.modelcontextprotocol.io).

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use mcpal_core::ServerSpec;
use serde::Deserialize;

const DEFAULT_BASE: &str = "https://registry.modelcontextprotocol.io";

#[derive(Deserialize)]
pub struct Envelope {
    pub servers: Vec<ServerWrapper>,
}
#[derive(Deserialize)]
pub struct ServerWrapper {
    pub server: Server,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Server {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub packages: Vec<Package>,
    pub remotes: Vec<Remote>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Package {
    pub registry_type: String,
    pub identifier: String,
    pub version: Option<String>,
    pub transport: Option<Transport>,
    pub environment_variables: Vec<EnvVar>,
    pub package_arguments: Vec<Argument>,
    pub runtime_arguments: Vec<Argument>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Transport {
    pub r#type: String,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    pub is_required: bool,
    pub default: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Argument {
    pub value: Option<String>,
    pub default: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Remote {
    pub r#type: String,
    pub url: String,
}

#[derive(serde::Serialize)]
pub struct Hit<'a> {
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    pub kind: &'static str,
}

pub fn classify(s: &Server) -> &'static str {
    if !s.packages.is_empty() {
        "stdio"
    } else if !s.remotes.is_empty() {
        "http"
    } else {
        "unknown"
    }
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("mcpal/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()?)
}

pub async fn search(query: &str, limit: u32) -> Result<Envelope> {
    let base = std::env::var("MCPAL_REGISTRY_URL").unwrap_or_else(|_| DEFAULT_BASE.into());
    Ok(client()?
        .get(format!("{base}/v0/servers"))
        .query(&[("search", query), ("limit", &limit.to_string())])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

/// Registry's `?name=` doesn't actually filter, so we search and pick the
/// exact-name match.
pub async fn fetch(name: &str) -> Result<Server> {
    search(name, 20)
        .await?
        .servers
        .into_iter()
        .map(|w| w.server)
        .find(|s| s.name == name)
        .ok_or_else(|| {
            anyhow!("registry: no exact match for '{name}' (try `mcpal server search {name}`)")
        })
}

pub fn to_spec(server: &Server, extra_env: &BTreeMap<String, String>) -> Result<ServerSpec> {
    if let Some(pkg) = server
        .packages
        .iter()
        .find(|p| p.transport.as_ref().is_none_or(|t| t.r#type == "stdio"))
    {
        return stdio_from_package(pkg, extra_env);
    }
    if let Some(r) = server.remotes.iter().find(|r| r.r#type == "streamable-http") {
        return Ok(ServerSpec::Http {
            url: r.url.clone(),
            headers: BTreeMap::new(),
            auth: None,
        });
    }
    bail!(
        "registry server '{}' has no stdio package or streamable-http remote",
        server.name
    )
}

fn stdio_from_package(pkg: &Package, extra_env: &BTreeMap<String, String>) -> Result<ServerSpec> {
    let id = &pkg.identifier;
    let ver = pkg.version.as_deref().filter(|v| !v.is_empty());
    let (command, mut args): (&str, Vec<String>) = match pkg.registry_type.as_str() {
        "npm" => (
            "npx",
            vec![
                "-y".into(),
                ver.map_or_else(|| id.clone(), |v| format!("{id}@{v}")),
            ],
        ),
        "pypi" => ("uvx", vec![id.clone()]),
        "oci" => (
            "docker",
            vec!["run".into(), "--rm".into(), "-i".into(), id.clone()],
        ),
        other => bail!("unsupported registry_type '{other}'"),
    };
    let extra_vals = |xs: &[Argument]| -> Vec<String> {
        xs.iter()
            .filter_map(|a| a.value.clone().or_else(|| a.default.clone()))
            .collect()
    };
    args.extend(extra_vals(&pkg.package_arguments));
    args.extend(extra_vals(&pkg.runtime_arguments));

    let mut env: BTreeMap<String, String> = pkg
        .environment_variables
        .iter()
        .filter_map(|v| Some((v.name.clone(), v.default.clone()?)))
        .collect();
    env.extend(extra_env.iter().map(|(k, v)| (k.clone(), v.clone())));
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
    Ok(ServerSpec::Stdio {
        command: command.into(),
        args,
        env,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(body: &str) -> Server {
        serde_json::from_str::<Envelope>(body)
            .unwrap()
            .servers
            .pop()
            .unwrap()
            .server
    }

    #[test]
    fn parses_remote_envelope() {
        let s = parse(
            r#"{"servers":[{"server":{
                "name":"x/y","remotes":[{"type":"streamable-http","url":"https://a/mcp"}]}}]}"#,
        );
        assert_eq!(classify(&s), "http");
        assert!(matches!(to_spec(&s, &BTreeMap::new()).unwrap(), ServerSpec::Http { .. }));
    }

    #[test]
    fn parses_npm_package_and_builds_npx() {
        let s = parse(
            r#"{"servers":[{"server":{"name":"x/y","packages":[{
                "registryType":"npm","identifier":"@mcp/foo","version":"1.2.3",
                "transport":{"type":"stdio"},
                "environmentVariables":[{"name":"API_KEY","isRequired":true}]}]}}]}"#,
        );
        let mut extra = BTreeMap::new();
        extra.insert("API_KEY".into(), "k".into());
        let ServerSpec::Stdio { command, args, env } = to_spec(&s, &extra).unwrap() else {
            panic!("expected stdio");
        };
        assert_eq!(command, "npx");
        assert_eq!(args, vec!["-y", "@mcp/foo@1.2.3"]);
        assert_eq!(env.get("API_KEY"), Some(&"k".to_string()));
    }

    #[test]
    fn errors_when_required_env_missing() {
        let s = parse(
            r#"{"servers":[{"server":{"name":"x/y","packages":[{
                "registryType":"npm","identifier":"@mcp/foo",
                "environmentVariables":[{"name":"NEEDED","isRequired":true}]}]}}]}"#,
        );
        assert!(to_spec(&s, &BTreeMap::new()).unwrap_err().to_string().contains("NEEDED"));
    }
}
