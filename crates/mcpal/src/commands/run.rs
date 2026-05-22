//! `mcpal run <NAME>` — execute a saved call from `mcpal.yml`.

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow, bail};
use mcpal_core::rmcp::model::CallToolRequestParams;
use serde_json::{Map, Value};

use crate::collection::{Call, Collection, find_collection, template};
use crate::runtime::Ctx;

pub async fn run(
    name: String,
    dry_run: bool,
    params_override: Vec<String>,
    ctx: &Ctx,
) -> Result<()> {
    let cwd = std::env::current_dir().context("cwd")?;
    let path = find_collection(&cwd, ctx.collection_override.as_deref())?.ok_or_else(|| {
        anyhow!(
            "collection not found: no mcpal.yml from {} upward",
            cwd.display()
        )
    })?;
    let coll = Collection::load(&path)?;

    let call: &Call = coll.calls.get(&name).ok_or_else(|| {
        let avail: Vec<&String> = coll.calls.keys().collect();
        anyhow!("not found in mcpal config: call '{name}' (available: {avail:?})")
    })?;

    let profile_name = select_profile(ctx, &coll);
    let empty: BTreeMap<String, String> = BTreeMap::new();
    let profile_vars = match coll.profiles.get(&profile_name) {
        Some(p) => p,
        None if coll.profiles.is_empty() => &empty,
        None => bail!("profile '{profile_name}' not in collection"),
    };

    let mut params = call.params.clone();
    template::render(&mut params, profile_vars).map_err(|e| anyhow!("{e}"))?;

    if !params_override.is_empty() {
        let mut obj: Map<String, Value> = match params {
            Value::Object(m) => m,
            Value::Null => Map::new(),
            _ => bail!(
                "--params-override requires `params:` be an object; call '{name}' has a scalar/array"
            ),
        };
        for kv in &params_override {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| anyhow!("--params-override expects K=V: {kv}"))?;
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
        bail!("server returned tools/call result with isError: true");
    }
    Ok(())
}

/// `--profile NAME` > `MCPAL_PROFILE` (handled by clap default) > `default-profile:` > "default".
/// `ctx.profile` already collapses the first two (clap reads MCPAL_PROFILE via `env` attr).
/// Empty/"default" + a `default-profile:` in the file means the collection's default wins.
fn select_profile(ctx: &Ctx, coll: &Collection) -> String {
    if ctx.profile != "default" {
        return ctx.profile.clone();
    }
    if let Some(default) = &coll.default_profile {
        return default.clone();
    }
    "default".to_string()
}
