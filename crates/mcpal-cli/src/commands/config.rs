use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cli::ConfigAction;
use crate::config::Config;

pub fn run(action: ConfigAction, path: &Path) -> Result<()> {
    match action {
        ConfigAction::Path => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigAction::Show => {
            let cfg = Config::load(path)?;
            println!("{}", toml::to_string_pretty(&cfg).context("serialize")?);
            Ok(())
        }
        ConfigAction::Edit => {
            let editor = std::env::var("VISUAL")
                .or_else(|_| std::env::var("EDITOR"))
                .unwrap_or_else(|_| "vi".to_string());
            if !path.exists() {
                Config::default().save(path)?;
            }
            let status = Command::new(editor).arg(path).status().context("spawn editor")?;
            if !status.success() {
                bail!("editor exited non-zero");
            }
            Ok(())
        }
    }
}
