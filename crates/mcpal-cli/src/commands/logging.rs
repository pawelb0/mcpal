use anyhow::Result;
use mcpal_core::rmcp::model::SetLevelRequestParams;
use serde_json::json;

use crate::cli::{LogLevel, LoggingAction};
use crate::runtime::Ctx;

pub async fn run(action: LoggingAction, ctx: &Ctx) -> Result<()> {
    match action {
        LoggingAction::SetLevel { reference, level } => set_level(&reference, level, ctx).await,
    }
}

async fn set_level(reference: &str, level: LogLevel, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    client
        .set_level(SetLevelRequestParams::new(level.into()))
        .await?;
    ctx.render_one(&json!({"ok": true, "level": format!("{level:?}").to_lowercase()}))?;
    Ok(())
}
