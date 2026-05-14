use std::fs;
use std::io::Read;

use anyhow::{Context, Result, bail};
use mcpal_core::rmcp::model::CallToolRequestParams;
use mcpal_output::{emit_list, emit_one};
use serde_json::{Map, Value};

use crate::cli::ToolAction;
use crate::kv;
use crate::runtime::Ctx;

pub async fn run(action: ToolAction, ctx: &Ctx) -> Result<()> {
    match action {
        ToolAction::List { reference } => list(&reference, ctx).await,
        ToolAction::Describe { reference, name } => describe(&reference, &name, ctx).await,
        ToolAction::Call {
            reference,
            name,
            cli_input_json,
            args,
        } => call(&reference, &name, cli_input_json.as_deref(), &args, ctx).await,
    }
}

async fn list(reference: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let tools = client.list_all_tools().await?;
    emit_list(ctx.format, &tools, &["name", "description"], |t| {
        vec![
            t.name.to_string(),
            t.description.as_deref().unwrap_or("").into(),
        ]
    })?;
    Ok(())
}

async fn describe(reference: &str, name: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let tools = client.list_all_tools().await?;
    let tool = tools
        .iter()
        .find(|t| t.name == *name)
        .ok_or_else(|| anyhow::anyhow!("no tool named '{name}' on {reference}"))?;
    emit_one(ctx.format, tool)?;
    Ok(())
}

async fn call(
    reference: &str,
    name: &str,
    cli_input_json: Option<&str>,
    flag_args: &[String],
    ctx: &Ctx,
) -> Result<()> {
    let arguments = build_arguments(cli_input_json, flag_args)?;
    let (_, client) = ctx.open(reference).await?;

    let mut params = CallToolRequestParams::new(name.to_string());
    if !arguments.is_empty() {
        params = params.with_arguments(arguments);
    }
    let result = client.call_tool(params).await.context("tools/call")?;
    emit_one(ctx.format, &result)?;
    Ok(())
}

fn build_arguments(
    cli_input_json: Option<&str>,
    flag_args: &[String],
) -> Result<Map<String, Value>> {
    let mut out = Map::new();

    if let Some(source) = cli_input_json {
        let text = if source == "-" {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("read stdin")?;
            buf
        } else {
            fs::read_to_string(source).with_context(|| format!("read {source}"))?
        };
        merge_object(&mut out, &text, source)?;
    }

    out.extend(kv::parse_flag_args(flag_args.iter())?);
    Ok(out)
}

fn merge_object(into: &mut Map<String, Value>, text: &str, source: &str) -> Result<()> {
    let v: Value =
        serde_json::from_str(text).with_context(|| format!("parse JSON from {source}"))?;
    let Value::Object(obj) = v else {
        bail!("{source} must contain a JSON object");
    };
    for (k, val) in obj {
        into.insert(k, val);
    }
    Ok(())
}
