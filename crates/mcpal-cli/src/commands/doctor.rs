//! `mcpal debug doctor` — sanity-check the local environment.

use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::{Map, Value, json};

use crate::keyring::{self, Kind};
use crate::oauth;
use crate::runtime::Ctx;

const PROBE: &str = "__mcpal_doctor_probe__";

pub fn run(ctx: &Ctx) -> Result<()> {
    let mut issues: Vec<String> = Vec::new();

    let (kr_ok, kr_err) = match keyring_round_trip() {
        Ok(()) => (true, None),
        Err(e) => {
            issues.push(format!("keyring round-trip failed: {e}"));
            (false, Some(e))
        }
    };

    let mut servers = Map::new();
    for (alias, spec) in &ctx.cfg.server {
        let bearer = keyring::get(alias, Kind::Bearer).is_some();
        let oauth_blob = keyring::get(alias, Kind::Oauth).is_some();
        let access = oauth::current_access_token(alias).is_some();
        let kind = spec.kind();
        if kind == "http" && !bearer && !oauth_blob && std::env::var("MCPAL_BEARER").is_err() {
            issues.push(format!(
                "server '{alias}' is HTTP but has no stored credentials"
            ));
        }
        if oauth_blob && !access {
            issues.push(format!(
                "server '{alias}' has an oauth blob but no usable access token \
                 — try `mcpal auth refresh {alias}`"
            ));
        }
        servers.insert(
            alias.clone(),
            json!({
                "kind": kind, "bearer_stored": bearer,
                "oauth_stored": oauth_blob, "oauth_access_token_present": access,
            }),
        );
    }

    let mut discovery = Map::new();
    match ctx.discovered() {
        Ok(list) => {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for d in list {
                *counts.entry(d.source).or_default() += 1;
            }
            for (s, n) in counts {
                discovery.insert(s.into(), Value::from(n));
            }
        }
        Err(e) => issues.push(format!("discovery failed: {e:#}")),
    }

    let ok = issues.is_empty() && kr_ok;
    ctx.render_one(&json!({
        "ok": ok,
        "mcpal": { "version": env!("CARGO_PKG_VERSION") },
        "config": {
            "path": ctx.config_path.display().to_string(),
            "exists": ctx.config_path.exists(),
            "server_count": ctx.cfg.server.len(),
        },
        "keyring": { "backend": backend_name(), "read_write": kr_ok, "error": kr_err },
        "servers": servers,
        "discovery": discovery,
        "issues": issues,
    }))?;
    if !ok {
        anyhow::bail!("doctor found issues; see report above");
    }
    Ok(())
}

fn keyring_round_trip() -> Result<(), String> {
    let canary = "mcpal-doctor-canary";
    keyring::put(PROBE, Kind::Bearer, canary).map_err(|e| format!("put: {e:#}"))?;
    let got = keyring::get(PROBE, Kind::Bearer).ok_or("get returned None")?;
    keyring::delete(PROBE, Kind::Bearer).map_err(|e| format!("delete: {e:#}"))?;
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
