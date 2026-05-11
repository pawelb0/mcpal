use anyhow::{Result, anyhow};
use mcpal_core::rmcp::model::GetPromptRequestParams;
use mcpal_output::{emit_list, emit_one};
use serde_json::{Map, Value};

use crate::cli::PromptAction;
use crate::runtime::Ctx;

pub async fn run(action: PromptAction, ctx: &Ctx) -> Result<()> {
    match action {
        PromptAction::List { reference } => list(&reference, ctx).await,
        PromptAction::Get {
            reference,
            name,
            args,
        } => get(&reference, &name, &args, ctx).await,
    }
}

async fn list(reference: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let prompts = client.list_all_prompts().await?;
    emit_list(ctx.format, &prompts, &["name", "description"], |p| {
        vec![p.name.clone(), p.description.clone().unwrap_or_default()]
    })?;
    Ok(())
}

async fn get(reference: &str, name: &str, arg_pairs: &[String], ctx: &Ctx) -> Result<()> {
    let mut params = GetPromptRequestParams::new(name.to_string());
    if !arg_pairs.is_empty() {
        let mut map: Map<String, Value> = Map::new();
        for kv in arg_pairs {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| anyhow!("--arg expects K=V, got: {kv}"))?;
            map.insert(k.into(), Value::String(v.into()));
        }
        params = params.with_arguments(map);
    }

    let (_, client) = ctx.open(reference).await?;
    let result = client.get_prompt(params).await?;
    emit_one(ctx.format, &result)?;
    Ok(())
}
