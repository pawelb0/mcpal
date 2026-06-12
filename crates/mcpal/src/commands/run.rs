use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use mcpal_core::rmcp::model::CallToolRequestParams;
use serde_json::{Map, Value};

use crate::collection::{Call, Collection, find_collection, template};
use crate::exit::CliError;
use crate::runtime::Ctx;

pub async fn run(
    name: String,
    dry_run: bool,
    params_override: Vec<String>,
    ctx: &Ctx,
) -> Result<()> {
    let cwd = std::env::current_dir().context("cwd")?;
    let path = find_collection(&cwd, ctx.collection_override.as_deref())?.ok_or_else(|| {
        CliError::CollectionNotFound(format!("no mcpal.yml from {} upward", cwd.display()))
    })?;
    let coll = Collection::load(&path)?;

    let call: &Call = coll.calls.get(&name).ok_or_else(|| {
        let names: Vec<&str> = coll.calls.keys().map(String::as_str).collect();
        CliError::NotFound(format!(
            "not found in mcpal config: call '{name}' (available: {})",
            names.join(", ")
        ))
    })?;

    let profile_name = select_profile(ctx, &coll);
    let empty: BTreeMap<String, String> = BTreeMap::new();
    let profile_vars = match coll.profiles.get(profile_name) {
        Some(p) => p,
        None if coll.profiles.is_empty() => &empty,
        None => return Err(CliError::UnknownProfile(profile_name.into()).into()),
    };

    let mut params = call.params.clone();
    template::render(&mut params, profile_vars).map_err(anyhow::Error::new)?;

    if !params_override.is_empty() {
        let mut obj: Map<String, Value> = match params {
            Value::Object(m) => m,
            Value::Null => Map::new(),
            _ => {
                return Err(CliError::Usage(format!(
                    "--params-override requires `params:` be an object; call '{name}' has a scalar/array"
                ))
                .into());
            }
        };
        for kv in &params_override {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| CliError::Usage(format!("--params-override expects K=V: {kv}")))?;
            obj.insert(k.to_string(), Value::String(v.to_string()));
        }
        params = Value::Object(obj);
    }

    if dry_run {
        ctx.render_one(&serde_json::json!({
            "dry_run": true,
            "server": call.server,
            "tool": call.tool,
            "params": params,
        }))?;
        return Ok(());
    }

    let arguments = match params {
        Value::Object(m) => m,
        Value::Null => Map::new(),
        _ => bail!("`params:` must be an object for call '{name}'"),
    };

    let (_, client) = ctx.open(&call.server).await?;
    let mut req = CallToolRequestParams::new(call.tool.clone());
    if !arguments.is_empty() {
        req = req.with_arguments(arguments);
    }
    let result = ctx
        .under_deadline(client.call_tool(req))
        .await?
        .context("tools/call")?;
    ctx.render_one(&result)?;
    if result.is_error.unwrap_or(false) {
        return Err(CliError::ToolFailed.into());
    }
    Ok(())
}

// "default" is clap's sentinel — when ctx.profile equals it, defer to the
// collection's `default-profile:` key.
fn select_profile<'a>(ctx: &'a Ctx, coll: &'a Collection) -> &'a str {
    if ctx.profile != "default" {
        return &ctx.profile;
    }
    coll.default_profile.as_deref().unwrap_or("default")
}
