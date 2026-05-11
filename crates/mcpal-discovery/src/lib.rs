use std::path::{Path, PathBuf};

use anyhow::Result;
use mcpal_core::ServerSpec;
use serde::Serialize;

mod parse;
pub mod sources;

pub trait Source: Send + Sync {
    fn id(&self) -> &'static str;
    /// (path, scope) pairs. Non-existent paths are skipped silently by `discover`.
    fn paths(&self, ctx: &DiscoveryCtx) -> Vec<(PathBuf, Scope)>;
    fn parse(&self, path: &Path, scope: Scope, bytes: &[u8]) -> Result<Vec<DiscoveredServer>>;
}

pub struct DiscoveryCtx {
    pub home: PathBuf,
    pub cwd: PathBuf,
}

impl DiscoveryCtx {
    pub fn current() -> Result<Self> {
        let home = directories::BaseDirs::new()
            .ok_or_else(|| anyhow::anyhow!("no home directory"))?
            .home_dir()
            .to_path_buf();
        Ok(Self {
            home,
            cwd: std::env::current_dir()?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredServer {
    pub source: &'static str,
    pub source_path: PathBuf,
    pub name: String,
    pub spec: ServerSpec,
    pub scope: Scope,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Global,
    Project,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Global => "global",
            Self::Project => "project",
        })
    }
}

pub fn discover(ctx: &DiscoveryCtx) -> Vec<DiscoveredServer> {
    let mut out = Vec::new();
    for src in sources::registry() {
        for (path, scope) in src.paths(ctx) {
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    tracing::warn!("{}: {e}", path.display());
                    continue;
                }
            };
            match src.parse(&path, scope, &bytes) {
                Ok(mut items) => out.append(&mut items),
                Err(e) => tracing::warn!("{}: {e:#}", path.display()),
            }
        }
    }
    out
}
