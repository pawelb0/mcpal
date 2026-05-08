use anyhow::Result;

use crate::runtime::Ctx;

pub async fn run(
    _reference: &str,
    _method: &str,
    _params: Option<&str>,
    _ctx: &Ctx,
) -> Result<()> {
    anyhow::bail!(
        "raw JSON-RPC passthrough lands in M3 alongside the HTTP transport. \
         For now use the typed subcommands (tool, resource, prompt, ping)."
    )
}
