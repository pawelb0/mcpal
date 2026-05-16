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
use crate::runtime::Ctx;

pub async fn run(action: ServerAction, ctx: &Ctx) -> Result<()> {
    match action {
        ServerAction::List(args) => list(args, ctx),
        ServerAction::Show { reference } => {
            ctx.render_one(&resolve(&reference, ctx)?.spec)?;
            Ok(())
        }
        ServerAction::Add(args) => add(args, ctx),
        ServerAction::Remove { alias } => {
            let mut cfg = Config::load(&ctx.config_path)?;
            if cfg.server.remove(&alias).is_none() {
                bail!("server '{alias}' not found");
            }
            cfg.save(&ctx.config_path)?;
            eprintln!("removed server '{alias}'");
            Ok(())
        }
        ServerAction::Import(args) => import(args, ctx),
        ServerAction::Info { reference } => peer_field(&reference, "/serverInfo", ctx).await,
        ServerAction::Protocol { reference } => {
            peer_field(&reference, "/protocolVersion", ctx).await
        }
        ServerAction::Capabilities { reference } => {
            peer_field(&reference, "/capabilities", ctx).await
        }
        ServerAction::Instructions { reference } => {
            peer_field(&reference, "/instructions", ctx).await
        }
        ServerAction::Ping { reference } => {
            let (r, _) = ctx.open(&reference).await?;
            ctx.render_one(&json!({ "ref": r.display, "ok": true }))?;
            Ok(())
        }
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
    if !args.discovered {
        for (alias, spec) in &ctx.cfg.server {
            rows.push(Row {
                source: "mcpal".into(),
                alias: alias.clone(),
                kind: spec.kind().into(),
                detail: describe_spec(spec),
            });
        }
    }
    if args.discovered || args.all {
        for s in ctx.discovered()? {
            if let Some(f) = args.source.as_deref()
                && s.source != f
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

fn parse_env(kvs: &[String]) -> Result<BTreeMap<String, String>> {
    kvs.iter()
        .map(|kv| {
            kv.split_once('=')
                .map(|(k, v)| (k.into(), v.into()))
                .ok_or_else(|| anyhow!("--env requires K=V: {kv}"))
        })
        .collect()
}

fn add(args: ServerAddArgs, ctx: &Ctx) -> Result<()> {
    let (command, stdio_args) = match (args.stdio, args.command.split_first()) {
        (Some(_), Some(_)) => bail!("can't combine `--stdio` with a trailing `-- <cmd>`"),
        (Some(cmd), None) => (Some(cmd), args.args),
        (None, Some((c, rest))) => {
            if !args.args.is_empty() {
                bail!("can't combine `--arg` with a trailing `-- <cmd>`");
            }
            (Some(c.clone()), rest.to_vec())
        }
        (None, None) => (None, args.args),
    };
    let spec = match (command, args.http) {
        (Some(_), Some(_)) => bail!("--stdio/`-- cmd` and --http are mutually exclusive"),
        (Some(cmd), None) => ServerSpec::Stdio {
            command: cmd,
            args: stdio_args,
            env: parse_env(&args.env)?,
        },
        (None, Some(url)) => ServerSpec::Http {
            url,
            headers: BTreeMap::new(),
            auth: None,
        },
        (None, None) => bail!("provide a stdio command (`-- cmd args…`) or `--http <url>`"),
    };
    write_server(&ctx.config_path, &args.alias, spec)
}

fn import(args: ServerImportArgs, ctx: &Ctx) -> Result<()> {
    let found = ctx
        .discovered()?
        .iter()
        .find(|s| s.source == args.from && s.name == args.name)
        .ok_or_else(|| anyhow!("not found: {}:{}", args.from, args.name))?;
    let alias = args.alias.unwrap_or_else(|| found.name.clone());
    write_server(&ctx.config_path, &alias, found.spec.clone())
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
    let spec = registry::to_spec(&server, &parse_env(&args.env)?)?;
    let alias = args
        .alias
        .unwrap_or_else(|| default_alias(&server.name).into());
    write_server(&ctx.config_path, &alias, spec)?;
    eprintln!("installed {} as '{alias}'", server.name);
    Ok(())
}

fn write_server(path: &std::path::Path, alias: &str, spec: ServerSpec) -> Result<()> {
    let mut cfg = Config::load(path)?;
    if cfg.server.contains_key(alias) {
        bail!("server '{alias}' already exists");
    }
    cfg.server.insert(alias.into(), spec);
    cfg.save(path)?;
    eprintln!("added server '{alias}'");
    Ok(())
}

/// `io.github.foo/bar` → `bar`; otherwise the whole name.
fn default_alias(name: &str) -> &str {
    name.rsplit_once('/').map_or(name, |(_, t)| t)
}

async fn peer_field(reference: &str, pointer: &str, ctx: &Ctx) -> Result<()> {
    let (_, client) = ctx.open(reference).await?;
    let v = client
        .peer_info()
        .and_then(|i| serde_json::to_value(i).ok())
        .and_then(|v| v.pointer(pointer).cloned())
        .unwrap_or(json!(null));
    ctx.render_one(&v)?;
    Ok(())
}
