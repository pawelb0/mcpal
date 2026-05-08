use std::io;

use anyhow::Result;
use clap::CommandFactory;

use crate::cli::{Cli, Shell};

pub fn run(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(clap_complete::Shell::from(shell), &mut cmd, name, &mut io::stdout());
    Ok(())
}
