use anyhow::Result;
use mcpal_core::{Client, connect};
use mcpal_output::Format;

use crate::config::Config;
use crate::resolver::{ResolvedServer, resolve};

/// Per-invocation context shared across command handlers.
pub struct Ctx {
    pub cfg: Config,
    pub format: Format,
}

impl Ctx {
    pub async fn open(&self, reference: &str) -> Result<(ResolvedServer, Client)> {
        let resolved = resolve(reference, &self.cfg)?;
        let client = connect(&resolved.spec).await?;
        Ok((resolved, client))
    }
}
