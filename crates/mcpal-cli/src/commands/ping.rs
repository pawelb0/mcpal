use anyhow::Result;
use mcpal_output::emit_one;
use serde_json::json;

use crate::runtime::{Ctx, probe};

pub async fn run(reference: &str, ctx: &Ctx) -> Result<()> {
    let (r, client) = ctx.open(reference).await?;
    let p = probe(&client);
    emit_one(
        ctx.format,
        &json!({
            "ref": r.display,
            "ok": true,
            "server": { "name": p.name, "version": p.version },
            "peerInfo": p.info,
        }),
    )?;
    Ok(())
}
