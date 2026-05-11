use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use mcpal_core::{AuthSpec, ServerSpec};

use crate::config::Config;

#[derive(Debug)]
pub struct ResolvedServer {
    pub display: String,
    pub spec: ServerSpec,
}

/// Resolution order: config alias, http(s) URL, path to JSON spec.
pub fn resolve(reference: &str, cfg: &Config) -> Result<ResolvedServer> {
    if let Some(spec) = cfg.server.get(reference) {
        return Ok(ResolvedServer {
            display: reference.into(),
            spec: spec.clone(),
        });
    }

    if reference.starts_with("http://") || reference.starts_with("https://") {
        return Ok(ResolvedServer {
            display: reference.into(),
            spec: ServerSpec::Http {
                url: reference.into(),
                headers: BTreeMap::new(),
                auth: Some(AuthSpec::Oauth),
            },
        });
    }

    let p = Path::new(reference);
    if p.is_file() {
        let text = fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
        let spec: ServerSpec =
            serde_json::from_str(&text).with_context(|| format!("parse {}", p.display()))?;
        return Ok(ResolvedServer {
            display: p.display().to_string(),
            spec,
        });
    }

    bail!("server '{reference}' not found in config and is not a URL or path")
}
