use anyhow::Result;
use mcpal_core::ServerSpec;
use mcpal_output::emit_list;

use crate::runtime::Ctx;

pub fn run(source: Option<&str>, ctx: &Ctx) -> Result<()> {
    let all = ctx.discovered()?;
    let filtered: Vec<_> = match source {
        Some(s) => all.iter().filter(|d| d.source == s).cloned().collect(),
        None => all.to_vec(),
    };
    emit_list(
        ctx.format,
        &filtered,
        &["source", "name", "scope", "detail"],
        |s| {
            vec![
                s.source.into(),
                s.name.clone(),
                s.scope.to_string(),
                describe_spec(&s.spec),
            ]
        },
    )?;
    Ok(())
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
