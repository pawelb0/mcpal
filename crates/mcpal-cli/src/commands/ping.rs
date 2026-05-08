use anyhow::Result;

use crate::runtime::Ctx;

pub async fn run(_reference: &str, _ctx: &Ctx) -> Result<()> {
    anyhow::bail!("todo(M1): ping command")
}
