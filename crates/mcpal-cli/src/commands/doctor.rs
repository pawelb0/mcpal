//! `mcpal debug doctor` — sanity check the local environment. Inspired by
//! `fly doctor`. Verifies: config readable, keyring round-trips, every
//! known server resolves, OAuth tokens present where expected, discovery
//! sources reachable.

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::keyring::{self, Kind};
use crate::oauth;
use crate::runtime::Ctx;

const PROBE_REF: &str = "__mcpal_doctor_probe__";

#[derive(Serialize)]
struct Report {
    ok: bool,
    mcpal: McpalInfo,
    config: ConfigInfo,
    keyring: KeyringInfo,
    servers: Map<String, Value>,
    discovery: Map<String, Value>,
    issues: Vec<String>,
}

#[derive(Serialize)]
struct McpalInfo {
    version: &'static str,
}

#[derive(Serialize)]
struct ConfigInfo {
    path: String,
    exists: bool,
    server_count: usize,
}

#[derive(Serialize)]
struct KeyringInfo {
    backend: &'static str,
    read_write: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn run(ctx: &Ctx) -> Result<()> {
    let mut issues = Vec::new();

    let config = ConfigInfo {
        path: ctx.config_path.display().to_string(),
        exists: ctx.config_path.exists(),
        server_count: ctx.cfg.server.len(),
    };

    let keyring_info = check_keyring(&mut issues);

    let mut servers = Map::new();
    for (alias, spec) in &ctx.cfg.server {
        let bearer = keyring::get(alias, Kind::Bearer).is_some();
        let oauth_blob = keyring::get(alias, Kind::Oauth).is_some();
        let access_token = oauth::current_access_token(alias).is_some();
        let kind = spec.kind();
        if kind == "http" && !bearer && !oauth_blob && std::env::var("MCPAL_BEARER").is_err() {
            issues.push(format!(
                "server '{alias}' is HTTP but has no stored credentials \
                 (bearer/oauth) and MCPAL_BEARER is unset"
            ));
        }
        if oauth_blob && !access_token {
            issues.push(format!(
                "server '{alias}' has an oauth blob but no usable access token \
                 — try `mcpal auth refresh {alias}`"
            ));
        }
        servers.insert(
            alias.clone(),
            serde_json::json!({
                "kind": kind,
                "bearer_stored": bearer,
                "oauth_stored": oauth_blob,
                "oauth_access_token_present": access_token,
            }),
        );
    }

    let mut discovery = Map::new();
    match ctx.discovered() {
        Ok(list) => {
            let mut counts: std::collections::BTreeMap<&str, usize> =
                std::collections::BTreeMap::new();
            for d in list {
                *counts.entry(d.source).or_insert(0) += 1;
            }
            for (src, n) in counts {
                discovery.insert(src.into(), Value::from(n));
            }
        }
        Err(e) => {
            issues.push(format!("discovery failed: {e:#}"));
        }
    }

    let ok = issues.is_empty() && keyring_info.read_write;

    let report = Report {
        ok,
        mcpal: McpalInfo {
            version: env!("CARGO_PKG_VERSION"),
        },
        config,
        keyring: keyring_info,
        servers,
        discovery,
        issues,
    };

    ctx.render_one(&report)?;
    if !report.ok {
        anyhow::bail!("doctor found issues; see report above");
    }
    Ok(())
}

fn check_keyring(issues: &mut Vec<String>) -> KeyringInfo {
    let backend = backend_name();
    match keyring_round_trip() {
        Ok(()) => KeyringInfo {
            backend,
            read_write: true,
            error: None,
        },
        Err(e) => {
            issues.push(format!("keyring round-trip failed: {e}"));
            KeyringInfo {
                backend,
                read_write: false,
                error: Some(e),
            }
        }
    }
}

fn keyring_round_trip() -> Result<(), String> {
    let canary = "mcpal-doctor-canary";
    keyring::put(PROBE_REF, Kind::Bearer, canary).map_err(|e| format!("put: {e:#}"))?;
    let got =
        keyring::get(PROBE_REF, Kind::Bearer).ok_or_else(|| "get returned None".to_string())?;
    keyring::delete(PROBE_REF, Kind::Bearer).map_err(|e| format!("delete: {e:#}"))?;
    if got != canary {
        return Err(format!("read-back mismatch: got '{got}'"));
    }
    Ok(())
}

const fn backend_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos-keychain"
    } else if cfg!(target_os = "windows") {
        "windows-credential-manager"
    } else if cfg!(target_os = "linux") {
        "linux-secret-service"
    } else {
        "unknown"
    }
}
