use anyhow::Result;
use mcpal_core::rmcp::model::GetPromptRequestParams;
use mcpal_output::{emit_list, emit_one};

use crate::cli::PromptAction;
use crate::kv;
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
        params = params.with_arguments(kv::parse_pairs(arg_pairs, "arg")?);
    }

    let (_, client) = ctx.open(reference).await?;
    let result = client.get_prompt(params).await?;
    emit_one(ctx.format, &result)?;
    Ok(())
}
