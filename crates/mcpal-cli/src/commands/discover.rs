use anyhow::Result;
use mcpal_core::ServerSpec;

use crate::runtime::Ctx;

pub fn run(source: Option<&str>, ctx: &Ctx) -> Result<()> {
    let all = ctx.discovered()?;
    let filtered: Vec<_> = match source {
        Some(s) => all.iter().filter(|d| d.source == s).cloned().collect(),
        None => all.to_vec(),
    };
    ctx.render_list(&filtered)?;
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
