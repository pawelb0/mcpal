use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow, bail};
use comfy_table::Table;
use mcpal_core::ServerSpec;
use mcpal_output::{Format, emit_json, emit_jsonl};
use serde_json::json;

use crate::cli::{ServerAction, ServerAddArgs};
use crate::config::Config;
use crate::resolver::resolve;
use crate::runtime::Ctx;

pub async fn run(action: ServerAction, ctx: &Ctx) -> Result<()> {
    match action {
        ServerAction::List => list(ctx),
        ServerAction::Show { reference } => show(&reference, ctx),
        ServerAction::Add(args) => add(args, ctx),
        ServerAction::Remove { alias } => remove(&alias, ctx),
        ServerAction::Test { reference } => test(&reference, ctx).await,
    }
}

fn list(ctx: &Ctx) -> Result<()> {
    let rows: Vec<_> = ctx
        .cfg
        .server
        .iter()
        .map(|(alias, spec)| (alias.clone(), describe(spec)))
        .collect();

    match ctx.format {
        Format::Json => {
            let payload: Vec<_> = rows
                .iter()
                .map(|(a, (k, d))| json!({"alias": a, "kind": k, "detail": d}))
                .collect();
            emit_json(&payload)?;
        }
        Format::Jsonl => {
            for (a, (k, d)) in &rows {
                emit_jsonl(&json!({"alias": a, "kind": k, "detail": d}))?;
            }
        }
        Format::Yaml | Format::Human => {
            let mut table = Table::new();
            table.set_header(vec!["alias", "kind", "detail"]);
            for (a, (k, d)) in &rows {
                table.add_row(vec![a.as_str(), k, d.as_str()]);
            }
            println!("{table}");
        }
    }
    Ok(())
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

fn show(reference: &str, ctx: &Ctx) -> Result<()> {
    let r = resolve(reference, &ctx.cfg)?;
    match ctx.format {
        Format::Json | Format::Jsonl => emit_json(&r.spec)?,
        _ => {
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
                env.insert(k.to_string(), v.to_string());
            }
            ServerSpec::Stdio { command: cmd, args: args.args, env }
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
    let info = client.peer_info();
    let info_json = info.map(serde_json::to_value).transpose().context("encode peer info")?;
    let name = info_json
        .as_ref()
        .and_then(|v| v.pointer("/server_info/name").and_then(|n| n.as_str()))
        .unwrap_or("unknown");
    let version = info_json
        .as_ref()
        .and_then(|v| v.pointer("/server_info/version").and_then(|n| n.as_str()))
        .unwrap_or("?");

    match ctx.format {
        Format::Json | Format::Jsonl => {
            let payload = json!({
                "ref": r.display,
                "ok": true,
                "server": { "name": name, "version": version },
                "peer_info": info_json,
            });
            if matches!(ctx.format, Format::Jsonl) {
                emit_jsonl(&payload)?;
            } else {
                emit_json(&payload)?;
            }
        }
        _ => println!("ok: {} ({} {})", r.display, name, version),
    }

    client.cancel().await.ok();
    Ok(())
}
