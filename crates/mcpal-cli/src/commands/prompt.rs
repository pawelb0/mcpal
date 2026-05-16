use anyhow::Result;
use mcpal_core::rmcp::model::GetPromptRequestParams;

use serde::Serialize;

use crate::cli::PromptAction;
use crate::kv;
use crate::runtime::Ctx;

pub async fn run(action: PromptAction, ctx: &Ctx) -> Result<()> {
    match action {
        PromptAction::List {
            reference,
            names_only,
        } => list(&reference, names_only, ctx).await,
        PromptAction::Get {
            reference,
            name,
            args,
        } => get(&reference, &name, &args, ctx).await,
    }
}

#[derive(Serialize)]
struct PromptSummary<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    required: Vec<&'a str>,
}

async fn list(reference: &str, names_only: bool, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let prompts = ctx.under_deadline(client.list_all_prompts()).await??;
    if names_only {
        for p in &prompts {
            println!("{}", p.name);
        }
        return Ok(());
    }
    let summaries: Vec<PromptSummary<'_>> = prompts
        .iter()
        .map(|p| PromptSummary {
            name: &p.name,
            description: p.description.as_deref(),
            required: p
                .arguments
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|a| a.required.unwrap_or(false))
                .map(|a| a.name.as_str())
                .collect(),
        })
        .collect();
    ctx.render_list(&summaries)?;
    Ok(())
}

async fn get(reference: &str, name: &str, flag_args: &[String], ctx: &Ctx) -> Result<()> {
    let mut params = GetPromptRequestParams::new(name.to_string());
    if !flag_args.is_empty() {
        params = params.with_arguments(kv::parse_flag_args(flag_args.iter())?);
    }

    let (_, client) = ctx.open(reference).await?;
    let result = ctx.under_deadline(client.get_prompt(params)).await??;
    ctx.render_one(&result)?;
    Ok(())
}
