use anyhow::Result;
use mcpal_output::{Format, emit_one};
use serde_json::json;

use crate::runtime::{Ctx, probe};

pub async fn run(reference: &str, ctx: &Ctx) -> Result<()> {
    let (r, client) = ctx.open(reference).await?;
    let p = probe(&client);

    match ctx.format {
        Format::Json | Format::Jsonl => emit_one(
            ctx.format,
            &json!({
                "ref": r.display,
                "ok": true,
                "server": { "name": p.name, "version": p.version },
                "peerInfo": p.info,
            }),
        )?,
        _ => println!("ok: {} ({} {})", r.display, p.name, p.version),
    }
    Ok(())
}
