use anyhow::Result;
use mcpal_core::ServerSpec;
use mcpal_discovery::{Ctx as DCtx, DiscoveredServer, Scope, discover};
use mcpal_output::emit_list;

use crate::runtime::Ctx;

pub fn run(source: Option<&str>, ctx: &Ctx) -> Result<()> {
    let dctx = DCtx::current()?;
    let mut servers = discover(&dctx);
    if let Some(filter) = source {
        servers.retain(|s| s.source == filter);
    }
    render(&servers, ctx)
}

pub fn render(servers: &[DiscoveredServer], ctx: &Ctx) -> Result<()> {
    emit_list(
        ctx.format,
        servers,
        &["source", "name", "scope", "detail"],
        |s| {
            vec![
                s.source.into(),
                s.name.clone(),
                scope_label(s.scope).into(),
                describe_spec(&s.spec),
            ]
        },
    )?;
    Ok(())
}

pub fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "global",
        Scope::Project => "project",
    }
}

pub fn describe_spec(spec: &ServerSpec) -> String {
    match spec {
        ServerSpec::Stdio { command, args, .. } => {
            if args.is_empty() {
                command.clone()
            } else {
                format!("{} {}", command, args.join(" "))
            }
        }
        ServerSpec::Http { url, .. } => url.clone(),
    }
}
