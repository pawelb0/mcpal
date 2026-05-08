use anyhow::Result;

use crate::cli::ResourceAction;
use crate::runtime::Ctx;

pub async fn run(_action: ResourceAction, _ctx: &Ctx) -> Result<()> {
    anyhow::bail!("todo(M1): resource command")
}
