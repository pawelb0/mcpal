use anyhow::{Result, bail};

pub fn run() -> Result<()> {
    bail!(
        "raw JSON-RPC passthrough lands in M3 alongside the HTTP transport. \
         For now use the typed subcommands (tool, resource, prompt, ping)."
    )
}
