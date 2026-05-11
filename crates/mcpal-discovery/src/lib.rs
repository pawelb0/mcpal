use std::path::{Path, PathBuf};

use anyhow::Result;
use mcpal_core::ServerSpec;
use serde::Serialize;

mod parse;
pub mod sources;

pub trait Source: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    /// Files to read for this client; non-existent paths are silently skipped.
    fn paths(&self, ctx: &Ctx) -> Vec<PathBuf>;
    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<Vec<DiscoveredServer>>;
}

pub struct Ctx {
    pub home: PathBuf,
    pub cwd: PathBuf,
}

impl Ctx {
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

pub fn all_sources() -> Vec<Box<dyn Source>> {
    sources::registry()
}

/// Run every registered source against the current environment, collecting
/// successful parses and logging failures via `tracing::warn`.
pub fn discover(ctx: &Ctx) -> Vec<DiscoveredServer> {
    let mut out = Vec::new();
    for src in all_sources() {
        for path in src.paths(ctx) {
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    tracing::warn!("{}: {e}", path.display());
                    continue;
                }
            };
            match src.parse(&path, &bytes) {
                Ok(mut items) => out.append(&mut items),
                Err(e) => tracing::warn!("{}: {e:#}", path.display()),
            }
        }
    }
    out
}
