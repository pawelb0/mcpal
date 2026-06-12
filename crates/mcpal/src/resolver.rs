use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use mcpal_core::{AuthSpec, ServerSpec};
use mcpal_discovery::DiscoveredServer;

use crate::exit::CliError;
use crate::runtime::Ctx;

#[derive(Debug)]
pub struct ResolvedServer {
    pub display: String,
    pub spec: ServerSpec,
}

/// Resolution order:
///   1. mcpal-owned alias
///   2. `cmd:<command> [args]` ephemeral stdio
///   3. http(s) URL
///   4. path to a JSON spec file
///   5. `<source>:<name>` from discovery
///   6. bare `<name>` from discovery (unambiguous)
pub fn resolve(reference: &str, ctx: &Ctx) -> Result<ResolvedServer> {
    let auth_override = match ctx.auth_override.as_deref() {
        Some(mode) => Some(crate::runtime::parse_auth_override(mode)?),
        None => None,
    };
    resolve_with(
        reference,
        &ctx.cfg.server,
        ctx.discovered()?,
        auth_override.as_ref(),
    )
}

pub(crate) fn resolve_with(
    reference: &str,
    owned: &BTreeMap<String, ServerSpec>,
    discovered: &[DiscoveredServer],
    auth_override: Option<&Option<AuthSpec>>,
) -> Result<ResolvedServer> {
    if let Some(spec) = owned.get(reference) {
        return Ok(ResolvedServer {
            display: reference.into(),
            spec: spec.clone(),
        });
    }

    if let Some(rest) = reference.strip_prefix("cmd:") {
        let mut parts = rest.split_whitespace();
        let command = parts
            .next()
            .ok_or_else(|| CliError::Usage("cmd: needs a command after the prefix".into()))?;
        return Ok(ResolvedServer {
            display: reference.into(),
            spec: ServerSpec::Stdio {
                command: command.into(),
                args: parts.map(String::from).collect(),
                env: BTreeMap::new(),
            },
        });
    }

    if reference.starts_with("http://") || reference.starts_with("https://") {
        let auth = match auth_override {
            Some(o) => o.clone(),
            None => Some(AuthSpec::Oauth),
        };
        return Ok(ResolvedServer {
            display: reference.into(),
            spec: ServerSpec::Http {
                url: reference.into(),
                headers: BTreeMap::new(),
                auth,
            },
        });
    }

    let p = Path::new(reference);
    if p.is_file() {
        let text = fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
        let spec: ServerSpec =
            serde_json::from_str(&text).with_context(|| format!("parse {}", p.display()))?;
        return Ok(ResolvedServer {
            display: p.display().to_string(),
            spec,
        });
    }

    if let Some((src, name)) = reference.split_once(':')
        && let Some(d) = discovered
            .iter()
            .find(|s| s.source == src && s.name == name)
    {
        return Ok(ResolvedServer {
            display: reference.into(),
            spec: d.spec.clone(),
        });
    }

    let bare: Vec<_> = discovered.iter().filter(|s| s.name == reference).collect();
    match bare.as_slice() {
        [] => Err(CliError::NotFound(format!(
            "server '{reference}' not found (owned, cmd:, URL, path, or discovered)"
        ))
        .into()),
        [only] => Ok(ResolvedServer {
            display: format!("{}:{}", only.source, only.name),
            spec: only.spec.clone(),
        }),
        many => bail!(
            "'{reference}' is ambiguous — matches: {}",
            many.iter()
                .map(|m| format!("{}:{}", m.source, m.name))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcpal_discovery::Scope;
    use std::path::PathBuf;

    fn stdio(cmd: &str) -> ServerSpec {
        ServerSpec::Stdio {
            command: cmd.into(),
            args: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    fn disc(source: &'static str, name: &str, cmd: &str) -> DiscoveredServer {
        DiscoveredServer {
            source,
            source_path: PathBuf::from("/x"),
            name: name.into(),
            spec: stdio(cmd),
            scope: Scope::Global,
        }
    }

    #[test]
    fn owned_alias_wins_over_discovery() {
        let mut owned = BTreeMap::new();
        owned.insert("ev".into(), stdio("owned"));
        let discovered = vec![disc("cursor", "ev", "cursor-cmd")];
        let r = resolve_with("ev", &owned, &discovered, None).unwrap();
        assert_eq!(r.display, "ev");
        match r.spec {
            ServerSpec::Stdio { command, .. } => assert_eq!(command, "owned"),
            _ => panic!("expected stdio"),
        }
    }

    #[test]
    fn https_url_becomes_oauth_http_spec() {
        let r = resolve_with("https://x.example/mcp", &BTreeMap::new(), &[], None)
            .expect("resolve url");
        assert_eq!(r.display, "https://x.example/mcp");
        match r.spec {
            ServerSpec::Http { url, auth, .. } => {
                assert_eq!(url, "https://x.example/mcp");
                assert!(matches!(auth, Some(AuthSpec::Oauth)));
            }
            _ => panic!("expected http"),
        }
    }

    #[test]
    fn http_url_also_accepted() {
        let r = resolve_with("http://localhost:9/mcp", &BTreeMap::new(), &[], None).unwrap();
        assert!(matches!(r.spec, ServerSpec::Http { .. }));
    }

    #[test]
    fn source_prefixed_lookup() {
        let discovered = vec![disc("cursor", "linear", "a"), disc("zed", "linear", "b")];
        let r = resolve_with("zed:linear", &BTreeMap::new(), &discovered, None).unwrap();
        assert_eq!(r.display, "zed:linear");
        if let ServerSpec::Stdio { command, .. } = r.spec {
            assert_eq!(command, "b");
        }
    }

    #[test]
    fn bare_unambiguous_match() {
        let discovered = vec![disc("cursor", "linear", "a")];
        let r = resolve_with("linear", &BTreeMap::new(), &discovered, None).unwrap();
        assert_eq!(r.display, "cursor:linear");
    }

    #[test]
    fn bare_ambiguous_errors_listing_matches() {
        let discovered = vec![disc("cursor", "linear", "a"), disc("zed", "linear", "b")];
        let err = resolve_with("linear", &BTreeMap::new(), &discovered, None).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("ambiguous"));
        assert!(msg.contains("cursor:linear"));
        assert!(msg.contains("zed:linear"));
    }

    #[test]
    fn unknown_ref_message_lists_resolution_order() {
        let err = resolve_with("ghost", &BTreeMap::new(), &[], None).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("not found"));
        assert!(msg.contains("owned, cmd:, URL, path, or discovered"));
    }

    #[test]
    fn source_prefix_with_no_match_falls_through_to_bare() {
        let discovered = vec![disc("cursor", "linear", "a")];
        let r = resolve_with("cursor:missing", &BTreeMap::new(), &discovered, None);
        assert!(r.is_err(), "no entry for cursor:missing should error");
    }

    #[test]
    fn cmd_prefix_single_command() {
        let r = resolve_with("cmd:echo", &BTreeMap::new(), &[], None).unwrap();
        assert_eq!(r.display, "cmd:echo");
        if let ServerSpec::Stdio { command, args, .. } = r.spec {
            assert_eq!(command, "echo");
            assert!(args.is_empty());
        } else {
            panic!("expected stdio");
        }
    }

    #[test]
    fn cmd_prefix_splits_whitespace_into_args() {
        let r = resolve_with(
            "cmd:npx -y @mcp/server-everything",
            &BTreeMap::new(),
            &[],
            None,
        )
        .unwrap();
        if let ServerSpec::Stdio { command, args, .. } = r.spec {
            assert_eq!(command, "npx");
            assert_eq!(args, vec!["-y", "@mcp/server-everything"]);
        }
    }

    #[test]
    fn cmd_prefix_collapses_repeated_whitespace() {
        let r = resolve_with("cmd:npx  -y   @x", &BTreeMap::new(), &[], None).unwrap();
        if let ServerSpec::Stdio { args, .. } = r.spec {
            assert_eq!(args, vec!["-y", "@x"]);
        }
    }

    #[test]
    fn empty_cmd_prefix_errors() {
        let err = resolve_with("cmd:", &BTreeMap::new(), &[], None).unwrap_err();
        assert!(err.to_string().contains("needs a command"));
    }

    #[test]
    fn cmd_prefix_skips_path_check() {
        // "cmd:./local" should be ephemeral stdio, not parsed as filesystem path.
        let r = resolve_with("cmd:./local --flag", &BTreeMap::new(), &[], None).unwrap();
        if let ServerSpec::Stdio { command, args, .. } = r.spec {
            assert_eq!(command, "./local");
            assert_eq!(args, vec!["--flag"]);
        }
    }

    #[test]
    fn owned_alias_beats_cmd_prefix_on_exact_match() {
        let mut owned = BTreeMap::new();
        owned.insert("cmd:echo".into(), stdio("aliased"));
        let r = resolve_with("cmd:echo", &owned, &[], None).unwrap();
        if let ServerSpec::Stdio { command, .. } = r.spec {
            assert_eq!(command, "aliased");
        }
    }

    #[test]
    fn auth_override_anon_strips_oauth_default() {
        let r = resolve_with("https://x.example/mcp", &BTreeMap::new(), &[], Some(&None)).unwrap();
        if let ServerSpec::Http { auth, .. } = r.spec {
            assert!(auth.is_none(), "--auth none should leave auth=None");
        } else {
            panic!("expected http");
        }
    }

    #[test]
    fn auth_override_bearer_env_via_resolver() {
        let r = resolve_with(
            "https://x.example/mcp",
            &BTreeMap::new(),
            &[],
            Some(&Some(AuthSpec::BearerEnv {
                env: "GH_TOKEN".into(),
            })),
        )
        .unwrap();
        if let ServerSpec::Http { auth, .. } = r.spec {
            assert!(matches!(
                auth,
                Some(AuthSpec::BearerEnv { env }) if env == "GH_TOKEN"
            ));
        }
    }

    #[test]
    fn auth_override_ignored_for_stdio_refs() {
        // `cmd:` is stdio — auth override should not apply / corrupt the spec.
        let r = resolve_with("cmd:echo", &BTreeMap::new(), &[], Some(&None)).unwrap();
        assert!(matches!(r.spec, ServerSpec::Stdio { .. }));
    }

    #[test]
    fn json_spec_file_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(
            &path,
            r#"{"transport":"stdio","command":"npx","args":["-y","x"]}"#,
        )
        .unwrap();
        let p = path.to_str().unwrap();
        let r = resolve_with(p, &BTreeMap::new(), &[], None).unwrap();
        assert_eq!(r.display, p);
        if let ServerSpec::Stdio { command, args, .. } = r.spec {
            assert_eq!(command, "npx");
            assert_eq!(args, vec!["-y", "x"]);
        } else {
            panic!("expected stdio");
        }
    }
}
