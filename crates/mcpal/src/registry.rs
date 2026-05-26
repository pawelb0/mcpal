//! MCP Registry client (registry.modelcontextprotocol.io).

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Result, bail};
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

#[derive(Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_required")]
    pub is_required: bool,
    pub default: Option<String>,
}

impl Default for EnvVar {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            is_required: true,
            default: None,
        }
    }
}

fn default_required() -> bool {
    true
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

/// Per-var info pulled from the registry, used to prompt or print hints.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnvVarHint {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Result of converting a registry server into a ServerSpec: the spec
/// plus everything the caller needs to know about declared env vars.
#[derive(Debug, Clone)]
pub struct RequiredEnvHint {
    /// Every declared env var (satisfied or not).
    pub vars: Vec<EnvVarHint>,
    /// Names of required vars that are unsatisfied — caller must prompt or bail.
    pub missing: Vec<String>,
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
/// exact-name match with the highest semver.
pub async fn fetch(name: &str) -> Result<Server> {
    let env = search(name, 20).await?;
    let candidates: Vec<Server> = env.servers.into_iter().map(|w| w.server).collect();
    pick_latest(name, candidates)
}

fn pick_latest(name: &str, candidates: Vec<Server>) -> Result<Server> {
    let mut hits: Vec<Server> = candidates.into_iter().filter(|s| s.name == name).collect();
    if hits.is_empty() {
        return Err(anyhow::anyhow!(
            "registry: no exact match for '{name}' (try `mcpal server search {name}`)"
        ));
    }
    hits.sort_by(|a, b| {
        let av = a
            .version
            .as_deref()
            .and_then(|v| semver::Version::parse(v).ok());
        let bv = b
            .version
            .as_deref()
            .and_then(|v| semver::Version::parse(v).ok());
        av.cmp(&bv)
    });
    Ok(hits.pop().unwrap())
}

pub fn to_spec(
    server: &Server,
    extra_env: &BTreeMap<String, String>,
) -> Result<(ServerSpec, RequiredEnvHint)> {
    if let Some(pkg) = server
        .packages
        .iter()
        .find(|p| p.transport.as_ref().is_none_or(|t| t.r#type == "stdio"))
    {
        return stdio_from_package(pkg, extra_env);
    }
    if let Some(r) = server
        .remotes
        .iter()
        .find(|r| r.r#type == "streamable-http")
    {
        return Ok((
            ServerSpec::Http {
                url: r.url.clone(),
                headers: BTreeMap::new(),
                auth: None,
            },
            RequiredEnvHint {
                vars: Vec::new(),
                missing: Vec::new(),
            },
        ));
    }
    bail!(
        "registry server '{}' has no stdio package or streamable-http remote",
        server.name
    )
}

fn stdio_from_package(
    pkg: &Package,
    extra_env: &BTreeMap<String, String>,
) -> Result<(ServerSpec, RequiredEnvHint)> {
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

    let vars: Vec<EnvVarHint> = pkg
        .environment_variables
        .iter()
        .map(|v| EnvVarHint {
            name: v.name.clone(),
            description: v.description.clone(),
        })
        .collect();
    let missing: Vec<String> = pkg
        .environment_variables
        .iter()
        .filter(|v| v.is_required && !env.contains_key(&v.name))
        .map(|v| v.name.clone())
        .collect();

    Ok((
        ServerSpec::Stdio {
            command: command.into(),
            args,
            env,
        },
        RequiredEnvHint { vars, missing },
    ))
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
        let (spec, _) = to_spec(&s, &BTreeMap::new()).unwrap();
        assert!(matches!(spec, ServerSpec::Http { .. }));
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
        let (spec, _) = to_spec(&s, &extra).unwrap();
        let ServerSpec::Stdio { command, args, env } = spec else {
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
        let (_, hint) = to_spec(&s, &BTreeMap::new()).unwrap();
        assert!(hint.missing.contains(&"NEEDED".to_string()));
    }

    #[test]
    fn env_var_without_isrequired_is_required_by_default() {
        let body = r#"{ "name": "X", "description": "the X" }"#;
        let v: EnvVar = serde_json::from_str(body).unwrap();
        assert_eq!(v.name, "X");
        assert_eq!(v.description.as_deref(), Some("the X"));
        assert!(v.is_required, "should default to required");
    }

    #[test]
    fn explicit_is_required_false_is_honoured() {
        let body = r#"{ "name": "X", "isRequired": false }"#;
        let v: EnvVar = serde_json::from_str(body).unwrap();
        assert!(!v.is_required);
    }

    #[test]
    fn env_var_default_value_round_trips() {
        let body = r#"{ "name": "X", "default": "yo" }"#;
        let v: EnvVar = serde_json::from_str(body).unwrap();
        assert_eq!(v.default.as_deref(), Some("yo"));
        assert!(v.is_required); // default doesn't override requiredness
    }

    #[test]
    fn to_spec_returns_missing_when_required_unsupplied() {
        let body = r#"{"servers":[{"server":{"name":"x","packages":[{"registryType":"npm","identifier":"p","environmentVariables":[
            {"name":"NEEDED","description":"the thing"}
        ]}]}}]}"#;
        let s = parse(body);
        let (spec, hint) = to_spec(&s, &BTreeMap::new()).expect("to_spec");
        assert!(matches!(spec, ServerSpec::Stdio { .. }));
        assert_eq!(hint.missing, vec!["NEEDED"]);
        assert_eq!(hint.vars.len(), 1);
        assert_eq!(hint.vars[0].name, "NEEDED");
        assert_eq!(hint.vars[0].description.as_deref(), Some("the thing"));
    }

    #[test]
    fn to_spec_satisfied_by_extra_env() {
        let body = r#"{"servers":[{"server":{"name":"x","packages":[{"registryType":"npm","identifier":"p","environmentVariables":[
            {"name":"NEEDED"}
        ]}]}}]}"#;
        let s = parse(body);
        let mut extra = BTreeMap::new();
        extra.insert("NEEDED".to_string(), "abc".to_string());
        let (_, hint) = to_spec(&s, &extra).unwrap();
        assert!(hint.missing.is_empty());
    }

    #[test]
    fn to_spec_satisfied_by_registry_default() {
        let body = r#"{"servers":[{"server":{"name":"x","packages":[{"registryType":"npm","identifier":"p","environmentVariables":[
            {"name":"NEEDED","default":"baked"}
        ]}]}}]}"#;
        let s = parse(body);
        let (spec, hint) = to_spec(&s, &BTreeMap::new()).unwrap();
        assert!(hint.missing.is_empty());
        if let ServerSpec::Stdio { env, .. } = spec {
            assert_eq!(env.get("NEEDED").map(String::as_str), Some("baked"));
        } else {
            panic!("expected stdio spec");
        }
    }

    #[test]
    fn to_spec_skips_non_required_when_missing() {
        let body = r#"{"servers":[{"server":{"name":"x","packages":[{"registryType":"npm","identifier":"p","environmentVariables":[
            {"name":"OPTIONAL","isRequired":false}
        ]}]}}]}"#;
        let s = parse(body);
        let (_, hint) = to_spec(&s, &BTreeMap::new()).unwrap();
        assert!(
            hint.missing.is_empty(),
            "isRequired:false should not appear in missing"
        );
        assert_eq!(hint.vars.len(), 1);
    }
}

#[cfg(test)]
mod fetch_tests {
    use super::*;

    fn srv(name: &str, ver: &str) -> Server {
        Server {
            name: name.into(),
            version: Some(ver.into()),
            ..Default::default()
        }
    }

    #[test]
    fn picks_max_semver() {
        let candidates = vec![
            srv("io.github.x/y", "0.1.0"),
            srv("io.github.x/y", "0.1.4"),
            srv("io.github.x/y", "0.1.2"),
            srv("io.github.other/z", "9.9.9"),
            srv("io.github.x/y", "0.1.3"),
            srv("io.github.x/y", "0.1.1"),
        ];
        let chosen = pick_latest("io.github.x/y", candidates).unwrap();
        assert_eq!(chosen.version.as_deref(), Some("0.1.4"));
    }

    #[test]
    fn unparseable_version_loses_to_real() {
        let candidates = vec![srv("p", "not-semver"), srv("p", "0.0.1")];
        let chosen = pick_latest("p", candidates).unwrap();
        assert_eq!(chosen.version.as_deref(), Some("0.0.1"));
    }

    #[test]
    fn no_match_returns_err() {
        let candidates = vec![srv("a", "0.1.0")];
        assert!(pick_latest("b", candidates).is_err());
    }
}
