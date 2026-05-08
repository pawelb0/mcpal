use anyhow::Result;

use crate::cli::ToolAction;
use crate::runtime::Ctx;

pub async fn run(_action: ToolAction, _ctx: &Ctx) -> Result<()> {
    anyhow::bail!("todo(M1): tool command")
}
