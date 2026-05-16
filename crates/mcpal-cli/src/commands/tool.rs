use std::fs;
use std::io::Read;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use mcpal_core::{Client, rmcp::model::CallToolRequestParams};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::cli::ToolAction;
use crate::kv;
use crate::runtime::Ctx;

pub async fn run(action: ToolAction, ctx: &Ctx) -> Result<()> {
    match action {
        ToolAction::List {
            reference,
            names_only,
        } => list(&reference, names_only, ctx).await,
        ToolAction::Describe { reference, name } => {
            let (client, tool) = find_tool(&reference, &name, ctx).await?;
            ctx.render_one(&tool)?;
            drop(client);
            Ok(())
        }
        ToolAction::Template { reference, name } => {
            let (client, tool) = find_tool(&reference, &name, ctx).await?;
            let schema = serde_json::to_value(&*tool.input_schema).unwrap_or(Value::Null);
            ctx.render_one(&schema_example(&schema))?;
            drop(client);
            Ok(())
        }
        ToolAction::Call {
            reference,
            name,
            cli_input_json,
            params,
            skip_validation,
            args,
        } => {
            call(
                &reference,
                &name,
                cli_input_json.as_deref(),
                params.as_deref(),
                skip_validation,
                &args,
                ctx,
            )
            .await
        }
    }
}

#[derive(Serialize)]
struct ToolSummary<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    required: Vec<String>,
}

async fn list(reference: &str, names_only: bool, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let tools = ctx.under_deadline(client.list_all_tools()).await??;
    if names_only {
        for t in &tools {
            println!("{}", t.name);
        }
        return Ok(());
    }
    let summaries: Vec<ToolSummary<'_>> = tools
        .iter()
        .map(|t| ToolSummary {
            name: t.name.as_ref(),
            description: t.description.as_deref(),
            required: t
                .input_schema
                .get("required")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        })
        .collect();
    ctx.render_list(&summaries)?;
    Ok(())
}

async fn find_tool(
    reference: &str,
    name: &str,
    ctx: &Ctx,
) -> Result<(Client, mcpal_core::rmcp::model::Tool)> {
    let (_, client) = ctx.open(reference).await?;
    let tools = ctx.under_deadline(client.list_all_tools()).await??;
    let tool = tools
        .into_iter()
        .find(|t| t.name == *name)
        .ok_or_else(|| anyhow::anyhow!("no tool named '{name}' on {reference}"))?;
    Ok((client, tool))
}

fn schema_example(s: &Value) -> Value {
    match s.get("type").and_then(Value::as_str).unwrap_or("") {
        "object" => Value::Object(
            s.get("properties")
                .and_then(Value::as_object)
                .map(|p| p.iter().map(|(k, v)| (k.clone(), schema_example(v))).collect())
                .unwrap_or_default(),
        ),
        "array" => Value::Array(vec![
            s.get("items").map(schema_example).unwrap_or(Value::Null),
        ]),
        "string" => Value::String(String::new()),
        "integer" | "number" => Value::Number(0.into()),
        "boolean" => Value::Bool(false),
        _ => Value::Null,
    }
}

async fn call(
    reference: &str,
    name: &str,
    cli_input_json: Option<&str>,
    params: Option<&str>,
    skip_validation: bool,
    flag_args: &[String],
    ctx: &Ctx,
) -> Result<()> {
    let arguments = build_arguments(cli_input_json, params, flag_args)?;
    let (_, client) = ctx.open(reference).await?;

    if !skip_validation && !arguments.is_empty() {
        let tools = ctx.under_deadline(client.list_all_tools()).await??;
        let schema: Arc<_> = tools
            .iter()
            .find(|t| t.name == *name)
            .ok_or_else(|| anyhow::anyhow!("no tool named '{name}' on {reference}"))?
            .input_schema
            .clone();
        validate_args(&schema, &arguments)?;
    }

    let mut req = CallToolRequestParams::new(name.to_string());
    if !arguments.is_empty() {
        req = req.with_arguments(arguments);
    }
    let result = ctx
        .under_deadline(client.call_tool(req))
        .await?
        .context("tools/call")?;
    ctx.render_one(&result)?;
    Ok(())
}

fn validate_args(schema: &Map<String, Value>, arguments: &Map<String, Value>) -> Result<()> {
    let validator = jsonschema::validator_for(&Value::Object(schema.clone()))
        .context("schema validation: tool's inputSchema is not a valid JSON Schema")?;
    let issues: Vec<String> = validator
        .iter_errors(&Value::Object(arguments.clone()))
        .map(|e| {
            let p = e.instance_path().to_string();
            format!("{}: {e}", if p.is_empty() { "/" } else { &p })
        })
        .collect();
    if !issues.is_empty() {
        anyhow::bail!("schema validation failed:\n  - {}", issues.join("\n  - "));
    }
    Ok(())
}

fn build_arguments(
    cli_input_json: Option<&str>,
    params: Option<&str>,
    flag_args: &[String],
) -> Result<Map<String, Value>> {
    let mut out = Map::new();
    if let Some(s) = cli_input_json {
        merge(&mut out, &read_spec(s, BareIs::Path)?)?;
    }
    if let Some(s) = params {
        merge(&mut out, &read_spec(s, BareIs::Inline)?)?;
    }
    out.extend(kv::parse_flag_args(flag_args.iter())?);
    Ok(out)
}

enum BareIs {
    Path,
    Inline,
}

/// `-` = stdin, `@path` = file, else depends on `bare`.
fn read_spec(spec: &str, bare: BareIs) -> Result<(String, String)> {
    if spec == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).context("read stdin")?;
        return Ok((buf, "stdin".into()));
    }
    if let Some(path) = spec.strip_prefix('@') {
        return Ok((fs::read_to_string(path).with_context(|| format!("read {path}"))?, path.into()));
    }
    match bare {
        BareIs::Path => Ok((
            fs::read_to_string(spec).with_context(|| format!("read {spec}"))?,
            spec.into(),
        )),
        BareIs::Inline => Ok((spec.into(), "--params".into())),
    }
}

fn merge(into: &mut Map<String, Value>, (text, source): &(String, String)) -> Result<()> {
    let v: Value =
        serde_json::from_str(text).with_context(|| format!("parse JSON from {source}"))?;
    let Value::Object(obj) = v else {
        bail!("{source} must contain a JSON object");
    };
    into.extend(obj);
    Ok(())
}
