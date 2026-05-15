use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};
use mcpal_core::ServerSpec;
use mcpal_output::{emit_list, emit_one};
use serde::Serialize;
use serde_json::json;

use crate::cli::{ServerAction, ServerAddArgs, ServerImportArgs, ServerListArgs};
use crate::commands::discover::describe_spec;
use crate::config::Config;
use crate::resolver::resolve;
use crate::runtime::{Ctx, probe};

pub async fn run(action: ServerAction, ctx: &Ctx) -> Result<()> {
    match action {
        ServerAction::List(args) => list(args, ctx),
        ServerAction::Show { reference } => show(&reference, ctx),
        ServerAction::Add(args) => add(args, ctx),
        ServerAction::Remove { alias } => remove(&alias, ctx),
        ServerAction::Import(args) => import(args, ctx),
        ServerAction::Test { reference } => test(&reference, ctx).await,
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

    emit_list(ctx.format, &rows, &[], |_| Vec::new())?;
    Ok(())
}

fn show(reference: &str, ctx: &Ctx) -> Result<()> {
    let r = resolve(reference, ctx)?;
    emit_one(ctx.format, &r.spec)?;
    Ok(())
}

fn add(args: ServerAddArgs, ctx: &Ctx) -> Result<()> {
    let spec = match (args.stdio, args.http) {
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
                args: args.args,
                env,
            }
        }
        (None, Some(url)) => ServerSpec::Http {
            url,
            headers: BTreeMap::new(),
            auth: None,
        },
        (Some(_), Some(_)) => bail!("--stdio and --http are mutually exclusive"),
        (None, None) => bail!("provide --stdio or --http"),
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

async fn test(reference: &str, ctx: &Ctx) -> Result<()> {
    let (r, client) = ctx.open(reference).await?;
    let p = probe(&client);
    emit_one(
        ctx.format,
        &json!({
            "ref": r.display,
            "ok": true,
            "server": { "name": p.name, "version": p.version },
            "peerInfo": p.info,
        }),
    )?;
    Ok(())
}
