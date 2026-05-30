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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn stdio(cmd: &str, args: &[&str]) -> ServerSpec {
        ServerSpec::Stdio {
            command: cmd.into(),
            args: args.iter().map(|s| (*s).into()).collect(),
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn stdio_no_args_is_command_alone() {
        assert_eq!(describe_spec(&stdio("npx", &[])), "npx");
    }

    #[test]
    fn stdio_args_join_with_spaces() {
        assert_eq!(
            describe_spec(&stdio("npx", &["-y", "@mcp/foo"])),
            "npx -y @mcp/foo"
        );
    }

    #[test]
    fn http_returns_url() {
        let spec = ServerSpec::Http {
            url: "https://x.example/mcp".into(),
            headers: BTreeMap::new(),
            auth: None,
        };
        assert_eq!(describe_spec(&spec), "https://x.example/mcp");
    }
}
