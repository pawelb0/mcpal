use anyhow::Result;

use crate::cli::ServerAction;
use crate::runtime::Ctx;

pub async fn run(_action: ServerAction, _ctx: &Ctx) -> Result<()> {
    anyhow::bail!("todo(M1): server command")
}
