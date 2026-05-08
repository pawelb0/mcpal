use anyhow::{Context, Result};
use mcpal_output::{Format, emit_json, emit_jsonl};
use serde_json::json;

use crate::runtime::Ctx;

pub async fn run(reference: &str, ctx: &Ctx) -> Result<()> {
    let (r, client) = ctx.open(reference).await?;
    let info = client.peer_info();
    let info_json = info
        .map(serde_json::to_value)
        .transpose()
        .context("encode peer info")?;

    let name = info_json
        .as_ref()
        .and_then(|v| v.pointer("/serverInfo/name").and_then(|n| n.as_str()))
        .unwrap_or("unknown");
    let version = info_json
        .as_ref()
        .and_then(|v| v.pointer("/serverInfo/version").and_then(|n| n.as_str()))
        .unwrap_or("?");

    match ctx.format {
        Format::Json | Format::Jsonl => {
            let payload = json!({
                "ref": r.display,
                "ok": true,
                "server": { "name": name, "version": version },
                "peer_info": info_json,
            });
            if matches!(ctx.format, Format::Jsonl) {
                emit_jsonl(&payload)?;
            } else {
                emit_json(&payload)?;
            }
        }
        _ => println!("ok: {} ({} {})", r.display, name, version),
    }

    client.cancel().await.ok();
    Ok(())
}
