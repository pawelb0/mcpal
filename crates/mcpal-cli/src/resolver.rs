use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use mcpal_core::{AuthSpec, ServerSpec};
use mcpal_discovery::{Ctx as DCtx, discover};

use crate::config::Config;

#[derive(Debug)]
pub struct ResolvedServer {
    pub display: String,
    pub spec: ServerSpec,
}

/// Resolution order:
///   1. mcpal-owned alias (`mcpal server add` entry)
///   2. http(s) URL
///   3. path to JSON spec file
///   4. `<source>:<name>` (e.g. `cursor:linear`)
///   5. bare `<name>` from discovery if unambiguous
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

    let dctx = DCtx::current()?;
    let discovered = discover(&dctx);

    if let Some((src, name)) = reference.split_once(':')
        && let Some(d) = discovered
            .iter()
            .find(|s| s.source == src && s.name == name)
    {
        return Ok(ResolvedServer {
            display: reference.into(),
            spec: d.spec.clone(),
        });
    }

    let bare: Vec<_> = discovered.iter().filter(|s| s.name == reference).collect();
    match bare.as_slice() {
        [] => bail!("server '{reference}' not found (owned, URL, path, or discovered)"),
        [only] => Ok(ResolvedServer {
            display: format!("{}:{}", only.source, only.name),
            spec: only.spec.clone(),
        }),
        many => bail!(
            "'{reference}' is ambiguous — matches: {}",
            many.iter()
                .map(|m| format!("{}:{}", m.source, m.name))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
