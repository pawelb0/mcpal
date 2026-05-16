use std::fs;
use std::io::Read;

use anyhow::{Context, Result, bail};
use mcpal_core::rmcp::model::CallToolRequestParams;

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
        ToolAction::Describe { reference, name } => describe(&reference, &name, ctx).await,
        ToolAction::Template { reference, name } => template(&reference, &name, ctx).await,
        ToolAction::Call {
            reference,
            name,
            cli_input_json,
            params,
            args,
        } => {
            call(
                &reference,
                &name,
                cli_input_json.as_deref(),
                params.as_deref(),
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
            required: required_fields(&t.input_schema),
        })
        .collect();
    ctx.render_list(&summaries)?;
    Ok(())
}

async fn describe(reference: &str, name: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let tools = ctx.under_deadline(client.list_all_tools()).await??;
    let tool = tools
        .iter()
        .find(|t| t.name == *name)
        .ok_or_else(|| anyhow::anyhow!("no tool named '{name}' on {reference}"))?;
    ctx.render_one(tool)?;
    Ok(())
}

async fn template(reference: &str, name: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let tools = ctx.under_deadline(client.list_all_tools()).await??;
    let tool = tools
        .iter()
        .find(|t| t.name == *name)
        .ok_or_else(|| anyhow::anyhow!("no tool named '{name}' on {reference}"))?;
    let schema = serde_json::to_value(&*tool.input_schema).unwrap_or(Value::Null);
    let example = schema_example(&schema);
    ctx.render_one(&example)?;
    Ok(())
}

/// Generate an example value from a JSON Schema fragment.
fn schema_example(schema: &Value) -> Value {
    let ty = schema.get("type").and_then(Value::as_str).unwrap_or("");
    match ty {
        "object" => {
            let mut obj = serde_json::Map::new();
            if let Some(props) = schema.get("properties").and_then(Value::as_object) {
                for (k, v) in props {
                    obj.insert(k.clone(), schema_example(v));
                }
            }
            Value::Object(obj)
        }
        "array" => {
            let items = schema
                .get("items")
                .map(schema_example)
                .unwrap_or(Value::Null);
            Value::Array(vec![items])
        }
        "string" => Value::String(String::new()),
        "integer" | "number" => Value::Number(0.into()),
        "boolean" => Value::Bool(false),
        "null" => Value::Null,
        _ => Value::Null,
    }
}

async fn call(
    reference: &str,
    name: &str,
    cli_input_json: Option<&str>,
    params: Option<&str>,
    flag_args: &[String],
    ctx: &Ctx,
) -> Result<()> {
    let arguments = build_arguments(cli_input_json, params, flag_args)?;
    let (_, client) = ctx.open(reference).await?;

    let mut params = CallToolRequestParams::new(name.to_string());
    if !arguments.is_empty() {
        params = params.with_arguments(arguments);
    }
    let result = ctx
        .under_deadline(client.call_tool(params))
        .await?
        .context("tools/call")?;
    ctx.render_one(&result)?;
    Ok(())
}

fn required_fields(schema: &Map<String, Value>) -> Vec<String> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn build_arguments(
    cli_input_json: Option<&str>,
    params: Option<&str>,
    flag_args: &[String],
) -> Result<Map<String, Value>> {
    let mut out = Map::new();

    if let Some(spec) = cli_input_json {
        let (text, source) = read_spec(spec, BareIs::Path)?;
        merge_object(&mut out, &text, &source)?;
    }
    if let Some(spec) = params {
        let (text, source) = read_spec(spec, BareIs::Inline)?;
        merge_object(&mut out, &text, &source)?;
    }
    out.extend(kv::parse_flag_args(flag_args.iter())?);
    Ok(out)
}

/// What a bare (no `@`, no `-`) value means for a given flag.
enum BareIs {
    Path,
    Inline,
}

/// `-` reads stdin, `@path` reads a file, everything else depends on the flag.
fn read_spec(spec: &str, bare: BareIs) -> Result<(String, String)> {
    if spec == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("read stdin")?;
        return Ok((buf, "stdin".into()));
    }
    if let Some(path) = spec.strip_prefix('@') {
        return Ok((
            fs::read_to_string(path).with_context(|| format!("read {path}"))?,
            path.into(),
        ));
    }
    match bare {
        BareIs::Path => Ok((
            fs::read_to_string(spec).with_context(|| format!("read {spec}"))?,
            spec.into(),
        )),
        BareIs::Inline => Ok((spec.into(), "--params".into())),
    }
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
