use anyhow::{Result, anyhow};
use comfy_table::Table;
use mcpal_core::rmcp::model::GetPromptRequestParams;
use mcpal_output::{Format, emit_json, emit_jsonl};
use serde_json::{Map, Value};

use crate::cli::PromptAction;
use crate::runtime::Ctx;

pub async fn run(action: PromptAction, ctx: &Ctx) -> Result<()> {
    match action {
        PromptAction::List { reference } => list(&reference, ctx).await,
        PromptAction::Get { reference, name, args } => {
            get(&reference, &name, &args, ctx).await
        }
    }
}

async fn list(reference: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let prompts = client.list_all_prompts().await?;

    match ctx.format {
        Format::Json => emit_json(&prompts)?,
        Format::Jsonl => {
            for p in &prompts {
                emit_jsonl(p)?;
            }
        }
        _ => {
            let mut table = Table::new();
            table.set_header(vec!["name", "description"]);
            for p in &prompts {
                table.add_row(vec![
                    p.name.as_str(),
                    p.description.as_deref().unwrap_or(""),
                ]);
            }
            println!("{table}");
        }
    }
    client.cancel().await.ok();
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
            map.insert(k.to_string(), Value::String(v.to_string()));
        }
        params = params.with_arguments(map);
    }

    let (_, client) = ctx.open(reference).await?;
    let result = client.get_prompt(params).await?;

    match ctx.format {
        Format::Jsonl => emit_jsonl(&result)?,
        _ => emit_json(&result)?,
    }
    client.cancel().await.ok();
    Ok(())
}
