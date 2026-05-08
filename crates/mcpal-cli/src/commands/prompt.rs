use anyhow::Result;

use crate::cli::PromptAction;
use crate::runtime::Ctx;

pub async fn run(_action: PromptAction, _ctx: &Ctx) -> Result<()> {
    anyhow::bail!("todo(M1): prompt command")
}
