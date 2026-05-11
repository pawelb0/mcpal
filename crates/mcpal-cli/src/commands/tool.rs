use std::fs;
use std::io::Read;
use std::path::Path;

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
        ToolAction::Call {
            reference,
            name,
            args,
            args_file,
            stdin_json,
        } => {
            call(
                &reference,
                &name,
                &args,
                args_file.as_deref(),
                stdin_json,
                ctx,
            )
            .await
        }
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

async fn call(
    reference: &str,
    name: &str,
    arg_pairs: &[String],
    args_file: Option<&Path>,
    stdin_json: bool,
    ctx: &Ctx,
) -> Result<()> {
    let arguments = build_arguments(arg_pairs, args_file, stdin_json)?;
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
    arg_pairs: &[String],
    args_file: Option<&Path>,
    stdin_json: bool,
) -> Result<Map<String, Value>> {
    let mut out = Map::new();

    if stdin_json {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("read stdin")?;
        merge_object(&mut out, &buf, "stdin")?;
    }
    if let Some(p) = args_file {
        let text = fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
        merge_object(&mut out, &text, &p.display().to_string())?;
    }
    out.extend(kv::parse_pairs(arg_pairs, "arg")?);
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
