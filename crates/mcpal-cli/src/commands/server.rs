use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow, bail};
use mcpal_core::ServerSpec;
use mcpal_output::{Format, emit_list, emit_one};
use serde::Serialize;
use serde_json::json;

use crate::cli::{ServerAction, ServerAddArgs};
use crate::config::Config;
use crate::resolver::resolve;
use crate::runtime::{Ctx, probe};

pub async fn run(action: ServerAction, ctx: &Ctx) -> Result<()> {
    match action {
        ServerAction::List => list(ctx),
        ServerAction::Show { reference } => show(&reference, ctx),
        ServerAction::Add(args) => add(args, ctx),
        ServerAction::Remove { alias } => remove(&alias, ctx),
        ServerAction::Test { reference } => test(&reference, ctx).await,
    }
}

#[derive(Serialize)]
struct Row<'a> {
    alias: &'a str,
    kind: &'a str,
    detail: String,
}

fn describe(spec: &ServerSpec) -> (&'static str, String) {
    match spec {
        ServerSpec::Stdio { command, args, .. } => {
            let mut s = command.clone();
            if !args.is_empty() {
                s.push(' ');
                s.push_str(&args.join(" "));
            }
            ("stdio", s)
        }
        ServerSpec::Http { url, .. } => ("http", url.clone()),
    }
}

fn list(ctx: &Ctx) -> Result<()> {
    let rows: Vec<Row<'_>> = ctx
        .cfg
        .server
        .iter()
        .map(|(alias, spec)| {
            let (kind, detail) = describe(spec);
            Row {
                alias,
                kind,
                detail,
            }
        })
        .collect();

    emit_list(ctx.format, &rows, &["alias", "kind", "detail"], |r| {
        vec![r.alias.into(), r.kind.into(), r.detail.clone()]
    })?;
    Ok(())
}

fn show(reference: &str, ctx: &Ctx) -> Result<()> {
    let r = resolve(reference, &ctx.cfg)?;
    match ctx.format {
        Format::Json | Format::Jsonl => emit_one(ctx.format, &r.spec)?,
        Format::Human | Format::Yaml => {
            let toml_str = toml::to_string_pretty(&r.spec).context("serialize")?;
            println!("[{}]\n{toml_str}", r.display);
        }
    }
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
    println!("added server '{}'", args.alias);
    Ok(())
}

fn remove(alias: &str, ctx: &Ctx) -> Result<()> {
    let mut cfg = Config::load(&ctx.config_path)?;
    if cfg.server.remove(alias).is_none() {
        bail!("server '{alias}' not found");
    }
    cfg.save(&ctx.config_path)?;
    println!("removed server '{alias}'");
    Ok(())
}

async fn test(reference: &str, ctx: &Ctx) -> Result<()> {
    let (r, client) = ctx.open(reference).await?;
    let p = probe(&client);

    match ctx.format {
        Format::Json | Format::Jsonl => emit_one(
            ctx.format,
            &json!({
                "ref": r.display,
                "ok": true,
                "server": { "name": p.name, "version": p.version },
                "peerInfo": p.info,
            }),
        )?,
        _ => println!("ok: {} ({} {})", r.display, p.name, p.version),
    }
    Ok(())
}
