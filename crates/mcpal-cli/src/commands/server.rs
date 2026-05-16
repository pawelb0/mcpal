use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};
use mcpal_core::ServerSpec;
use serde::Serialize;
use serde_json::json;

use crate::cli::{
    ServerAction, ServerAddArgs, ServerImportArgs, ServerInstallArgs, ServerListArgs,
};
use crate::commands::discover::describe_spec;
use crate::config::Config;
use crate::registry;
use crate::resolver::resolve;
use crate::runtime::{Ctx, probe};

pub async fn run(action: ServerAction, ctx: &Ctx) -> Result<()> {
    match action {
        ServerAction::List(args) => list(args, ctx),
        ServerAction::Show { reference } => show(&reference, ctx),
        ServerAction::Add(args) => add(args, ctx),
        ServerAction::Remove { alias } => remove(&alias, ctx),
        ServerAction::Import(args) => import(args, ctx),
        ServerAction::Test { reference, full } => test(&reference, full, ctx).await,
        ServerAction::Search { keywords, limit } => search(&keywords, limit, ctx).await,
        ServerAction::Install(args) => install(args, ctx).await,
        ServerAction::Discover { source } => crate::commands::discover::run(source.as_deref(), ctx),
    }
}

#[derive(Serialize)]
struct Row {
    source: String,
    alias: String,
    kind: String,
    detail: String,
}

fn list(args: ServerListArgs, ctx: &Ctx) -> Result<()> {
    let mut rows: Vec<Row> = Vec::new();
    let include_owned = !args.discovered;
    let include_discovered = args.discovered || args.all;

    if include_owned {
        for (alias, spec) in &ctx.cfg.server {
            rows.push(Row {
                source: "mcpal".into(),
                alias: alias.clone(),
                kind: spec.kind().into(),
                detail: describe_spec(spec),
            });
        }
    }

    if include_discovered {
        for s in ctx.discovered()? {
            if let Some(filter) = args.source.as_deref()
                && s.source != filter
            {
                continue;
            }
            rows.push(Row {
                source: s.source.into(),
                alias: s.name.clone(),
                kind: s.spec.kind().into(),
                detail: describe_spec(&s.spec),
            });
        }
    }

    ctx.render_list(&rows)?;
    Ok(())
}

fn show(reference: &str, ctx: &Ctx) -> Result<()> {
    let r = resolve(reference, ctx)?;
    ctx.render_one(&r.spec)?;
    Ok(())
}

fn add(args: ServerAddArgs, ctx: &Ctx) -> Result<()> {
    let mut command = args.stdio;
    let mut stdio_args = args.args;
    if let Some((first, rest)) = args.command.split_first() {
        if command.is_some() {
            bail!("can't combine `--stdio` with a trailing `-- <cmd>`; pick one form");
        }
        command = Some(first.clone());
        if !stdio_args.is_empty() {
            bail!("can't combine `--arg` with a trailing `-- <cmd>`; pick one form");
        }
        stdio_args = rest.to_vec();
    }

    let spec = match (command, args.http) {
        (Some(cmd), None) => {
            let mut env = BTreeMap::new();
            for kv in args.env {
                let (k, v) = kv
                    .split_once('=')
                    .ok_or_else(|| anyhow!("--env requires K=V: {kv}"))?;
                env.insert(k.into(), v.into());
            }
            ServerSpec::Stdio {
                command: cmd,
                args: stdio_args,
                env,
            }
        }
        (None, Some(url)) => ServerSpec::Http {
            url,
            headers: BTreeMap::new(),
            auth: None,
        },
        (Some(_), Some(_)) => bail!("--stdio/`-- cmd` and --http are mutually exclusive"),
        (None, None) => bail!("provide a stdio command (`-- cmd args…`) or `--http <url>`"),
    };

    let mut cfg = Config::load(&ctx.config_path)?;
    if cfg.server.contains_key(&args.alias) {
        bail!("server '{}' already exists", args.alias);
    }
    cfg.server.insert(args.alias.clone(), spec);
    cfg.save(&ctx.config_path)?;
    eprintln!("added server '{}'", args.alias);
    Ok(())
}

fn remove(alias: &str, ctx: &Ctx) -> Result<()> {
    let mut cfg = Config::load(&ctx.config_path)?;
    if cfg.server.remove(alias).is_none() {
        bail!("server '{alias}' not found");
    }
    cfg.save(&ctx.config_path)?;
    eprintln!("removed server '{alias}'");
    Ok(())
}

fn import(args: ServerImportArgs, ctx: &Ctx) -> Result<()> {
    let found = ctx
        .discovered()?
        .iter()
        .find(|s| s.source == args.from && s.name == args.name)
        .ok_or_else(|| anyhow!("not found: {}:{}", args.from, args.name))?;

    let alias = args.alias.unwrap_or_else(|| found.name.clone());
    let mut cfg = Config::load(&ctx.config_path)?;
    if cfg.server.contains_key(&alias) {
        bail!("server '{alias}' already exists in mcpal config");
    }
    cfg.server.insert(alias.clone(), found.spec.clone());
    cfg.save(&ctx.config_path)?;
    eprintln!("imported {}:{} as '{alias}'", found.source, found.name);
    Ok(())
}

async fn search(keywords: &str, limit: u32, ctx: &Ctx) -> Result<()> {
    let env = registry::search(keywords, limit).await?;
    let hits: Vec<registry::Hit<'_>> = env
        .servers
        .iter()
        .map(|w| registry::Hit {
            name: &w.server.name,
            version: w.server.version.as_deref(),
            description: w.server.description.as_deref(),
            kind: registry::classify(&w.server),
        })
        .collect();
    ctx.render_list(&hits)?;
    Ok(())
}

async fn install(args: ServerInstallArgs, ctx: &Ctx) -> Result<()> {
    let server = registry::fetch(&args.name).await?;
    let mut env_map = BTreeMap::new();
    for kv in &args.env {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| anyhow!("--env requires K=V: {kv}"))?;
        env_map.insert(k.into(), v.into());
    }
    let spec = registry::to_spec(&server, &env_map)?;

    let alias = args
        .alias
        .unwrap_or_else(|| default_alias(&server.name).to_string());
    let mut cfg = Config::load(&ctx.config_path)?;
    if cfg.server.contains_key(&alias) {
        bail!("server '{alias}' already exists in mcpal config");
    }
    cfg.server.insert(alias.clone(), spec);
    cfg.save(&ctx.config_path)?;
    eprintln!("installed {} as '{alias}'", server.name);
    Ok(())
}

/// `io.github.foo/bar` → `bar`; falls back to the whole name if there's
/// no `/`.
fn default_alias(name: &str) -> &str {
    name.rsplit_once('/').map(|(_, t)| t).unwrap_or(name)
}

async fn test(reference: &str, full: bool, ctx: &Ctx) -> Result<()> {
    let (r, client) = ctx.open(reference).await?;
    let p = probe(&client);
    let mut out = json!({
        "ref": r.display,
        "ok": true,
        "server": { "name": p.name, "version": p.version },
        "peerInfo": p.info,
    });
    if full {
        let tools = ctx.under_deadline(client.list_all_tools()).await?.ok();
        let resources = ctx.under_deadline(client.list_all_resources()).await?.ok();
        let prompts = ctx.under_deadline(client.list_all_prompts()).await?.ok();
        out["capabilities"] = json!({
            "tools": tools.as_ref().map(Vec::len).unwrap_or(0),
            "resources": resources.as_ref().map(Vec::len).unwrap_or(0),
            "prompts": prompts.as_ref().map(Vec::len).unwrap_or(0),
        });
    }
    ctx.render_one(&out)?;
    Ok(())
}
