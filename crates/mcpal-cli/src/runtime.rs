use std::path::PathBuf;

use anyhow::Result;
use mcpal_core::{Client, connect};
use mcpal_output::Format;

use crate::config::Config;
use crate::resolver::{ResolvedServer, resolve};

pub struct Ctx {
    pub cfg: Config,
    pub format: Format,
    pub config_path: PathBuf,
}

impl Ctx {
    pub async fn open(&self, reference: &str) -> Result<(ResolvedServer, Client)> {
        let resolved = resolve(reference, &self.cfg)?;
        let client = connect(&resolved.spec).await?;
        Ok((resolved, client))
    }
}
