use std::path::Path;

use anyhow::{Result, bail};

use crate::config::Config;

pub fn run(path: &Path) -> Result<()> {
    if path.exists() {
        bail!("config already exists at {}", path.display());
    }
    Config::default().save(path)?;
    println!("wrote {}", path.display());
    Ok(())
}
