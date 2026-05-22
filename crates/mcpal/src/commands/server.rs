use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};
use mcpal_core::{AuthSpec, ServerSpec};

use crate::keyring;
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
        ServerAction::Add(args) => add(args, ctx).await,
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

async fn add(args: ServerAddArgs, ctx: &Ctx) -> Result<()> {
    let alias = args.alias.clone();
    let no_login = args.no_login;
    let force = args.force;
    let (spec, intent) = derive(args)?;
    let transport = match &spec {
        ServerSpec::Http { .. } => "http",
        ServerSpec::Stdio { .. } => "stdio",
    };
    write_server(&ctx.config_path, &alias, spec, force)?;
    materialise_auth(&alias, &intent, no_login, ctx).await?;
    ctx.render_one(&json!({
        "ok": true,
        "ref": alias,
        "transport": transport,
        "auth": auth_label(&intent),
    }))?;
    Ok(())
}

fn auth_label(intent: &AuthIntent) -> &'static str {
    match intent {
        AuthIntent::None => "none",
        AuthIntent::Literal(_) => "bearer",
        AuthIntent::Env(_) => "bearer_env",
        AuthIntent::Oauth => "oauth",
    }
}

async fn materialise_auth(
    alias: &str,
    intent: &AuthIntent,
    no_login: bool,
    _ctx: &Ctx,
) -> Result<()> {
    match intent {
        AuthIntent::None => Ok(()),
        AuthIntent::Literal(token) => {
            let token = if token == "-" {
                crate::commands::auth::read_stdin()?
            } else {
                token.clone()
            };
            if token.is_empty() {
                bail!("--bearer - read an empty token from stdin");
            }
            keyring::put(alias, keyring::Kind::Bearer, &token)?;
            Ok(())
        }
        AuthIntent::Env(_) => Ok(()), // spec already carries bearer_env
        AuthIntent::Oauth => {
            let _ = no_login;
            // Filled out in Task 3.
            Ok(())
        }
    }
}

fn import(args: ServerImportArgs, ctx: &Ctx) -> Result<()> {
    let found = ctx
        .discovered()?
        .iter()
        .find(|s| s.source == args.from && s.name == args.name)
        .ok_or_else(|| anyhow!("not found: {}:{}", args.from, args.name))?;
    let alias = args.alias.unwrap_or_else(|| found.name.clone());
    let mut spec = found.spec.clone();
    promote_auth(&mut spec, &alias)?;
    write_server(&ctx.config_path, &alias, spec, false)
}

#[derive(Debug, PartialEq, Eq)]
enum BearerSource {
    None,
    Literal(String),
    Env(String),
}

#[derive(Debug, PartialEq, Eq)]
enum AuthIntent {
    None,
    Literal(String),
    Env(String),
    Oauth,
}

// Pure derivation of what add() will persist. No I/O.
fn derive(args: ServerAddArgs) -> Result<(ServerSpec, AuthIntent)> {
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
    let is_stdio = command.is_some();
    let auth_flags_present = args.bearer.is_some()
        || args.bearer_env.is_some()
        || args.oauth
        || args.header.iter().any(|h| {
            h.split_once(':')
                .is_some_and(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        });
    if is_stdio && auth_flags_present {
        bail!("auth flags require --http (stdio servers carry no Authorization)");
    }

    let mut spec = match (command, args.http) {
        (Some(_), Some(_)) => bail!("--stdio/`-- cmd` and --http are mutually exclusive"),
        (Some(cmd), None) => ServerSpec::Stdio {
            command: cmd,
            args: stdio_args,
            env: parse_env(&args.env)?,
        },
        (None, Some(url)) => {
            let mut headers = BTreeMap::new();
            for h in &args.header {
                let (k, v) = h
                    .split_once(':')
                    .ok_or_else(|| anyhow!("--header needs `K: V`, got: {h}"))?;
                headers.insert(k.trim().to_string(), v.trim().to_string());
            }
            ServerSpec::Http {
                url,
                headers,
                auth: None,
            }
        }
        (None, None) => bail!("provide a stdio command (`-- cmd args…`) or `--http <url>`"),
    };

    // 1) header-derived Authorization wins as the *baseline*.
    let header_intent = match extract_bearer(&mut spec) {
        BearerSource::None => AuthIntent::None,
        BearerSource::Literal(t) => AuthIntent::Literal(t),
        BearerSource::Env(v) => AuthIntent::Env(v),
    };

    // 2) explicit --bearer / --bearer-env / --oauth override the header path.
    let intent = if let Some(t) = args.bearer {
        AuthIntent::Literal(t)
    } else if let Some(v) = args.bearer_env {
        if let ServerSpec::Http { auth, .. } = &mut spec {
            *auth = Some(AuthSpec::BearerEnv { env: v.clone() });
        }
        AuthIntent::Env(v)
    } else if args.oauth {
        if let ServerSpec::Http { auth, .. } = &mut spec {
            *auth = Some(AuthSpec::Oauth);
        }
        AuthIntent::Oauth
    } else {
        header_intent
    };

    Ok((spec, intent))
}

/// Strip any `Authorization: Bearer …` header out of an HTTP spec and
/// classify it. Literal tokens come back as `Literal`; `${VAR}` / `$VAR`
/// references mutate `auth` to `BearerEnv` and come back as `Env`. Any
/// other Authorization value (Basic, Digest, …) is left in place.
fn extract_bearer(spec: &mut ServerSpec) -> BearerSource {
    let ServerSpec::Http { headers, auth, .. } = spec else {
        return BearerSource::None;
    };
    let Some(key) = headers
        .keys()
        .find(|k| k.eq_ignore_ascii_case("authorization"))
        .cloned()
    else {
        return BearerSource::None;
    };
    let value = headers.remove(&key).unwrap_or_default();
    let Some(token) = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
    else {
        headers.insert(key, value);
        return BearerSource::None;
    };
    let env_var = token
        .strip_prefix("${")
        .and_then(|s| s.strip_suffix('}'))
        .or_else(|| token.strip_prefix('$'))
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_'));
    if let Some(env) = env_var {
        *auth = Some(AuthSpec::BearerEnv {
            env: env.to_string(),
        });
        return BearerSource::Env(env.to_string());
    }
    BearerSource::Literal(token.to_string())
}

/// Pull any `Authorization: Bearer …` header out of an imported HTTP spec.
/// Literal tokens go to the OS keyring; `${ENV}` references convert to
/// `BearerEnv`. Either way, no secret lands in `config.toml`.
fn promote_auth(spec: &mut ServerSpec, alias: &str) -> Result<()> {
    match extract_bearer(spec) {
        BearerSource::None => {}
        BearerSource::Literal(token) => {
            keyring::put(alias, keyring::Kind::Bearer, &token)?;
            eprintln!("imported '{alias}': bearer stored in keyring");
        }
        BearerSource::Env(env) => {
            eprintln!("imported '{alias}': bearer comes from ${env}");
        }
    }
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
    let spec = registry::to_spec(&server, &parse_env(&args.env)?)?;
    let alias = args
        .alias
        .unwrap_or_else(|| default_alias(&server.name).into());
    write_server(&ctx.config_path, &alias, spec, false)?;
    eprintln!("installed {} as '{alias}'", server.name);
    Ok(())
}

fn write_server(path: &std::path::Path, alias: &str, spec: ServerSpec, force: bool) -> Result<()> {
    let mut cfg = Config::load(path)?;
    if cfg.server.contains_key(alias) && !force {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn args(alias: &str) -> crate::cli::ServerAddArgs {
        crate::cli::ServerAddArgs {
            alias: alias.into(),
            stdio: None,
            args: vec![],
            env: vec![],
            http: None,
            bearer: None,
            bearer_env: None,
            oauth: false,
            header: vec![],
            no_login: false,
            force: false,
            command: vec![],
        }
    }

    #[test]
    fn intent_none_when_no_auth_flags() {
        let mut a = args("x");
        a.http = Some("https://x".into());
        let (spec, intent) = derive(a).expect("derive");
        assert!(matches!(intent, AuthIntent::None));
        assert!(matches!(spec, ServerSpec::Http { .. }));
    }

    #[test]
    fn intent_literal_from_bearer_flag() {
        let mut a = args("x");
        a.http = Some("https://x".into());
        a.bearer = Some("abc".into());
        let (_, intent) = derive(a).expect("derive");
        assert!(matches!(intent, AuthIntent::Literal(ref t) if t == "abc"));
    }

    #[test]
    fn intent_env_from_bearer_env_flag() {
        let mut a = args("x");
        a.http = Some("https://x".into());
        a.bearer_env = Some("GH_TOKEN".into());
        let (spec, intent) = derive(a).expect("derive");
        assert!(matches!(intent, AuthIntent::Env(ref v) if v == "GH_TOKEN"));
        if let ServerSpec::Http { auth, .. } = spec {
            assert!(matches!(
                auth,
                Some(mcpal_core::AuthSpec::BearerEnv { env }) if env == "GH_TOKEN"
            ));
        } else {
            panic!("expected http spec");
        }
    }

    #[test]
    fn intent_oauth_from_oauth_flag() {
        let mut a = args("x");
        a.http = Some("https://x".into());
        a.oauth = true;
        let (spec, intent) = derive(a).expect("derive");
        assert!(matches!(intent, AuthIntent::Oauth));
        if let ServerSpec::Http { auth, .. } = spec {
            assert!(matches!(auth, Some(mcpal_core::AuthSpec::Oauth)));
        }
    }

    #[test]
    fn header_authorization_bearer_promotes_to_literal() {
        let mut a = args("x");
        a.http = Some("https://x".into());
        a.header = vec!["Authorization: Bearer abc".into()];
        let (spec, intent) = derive(a).expect("derive");
        assert!(matches!(intent, AuthIntent::Literal(ref t) if t == "abc"));
        if let ServerSpec::Http { headers, .. } = spec {
            assert!(
                !headers
                    .keys()
                    .any(|k| k.eq_ignore_ascii_case("authorization"))
            );
        }
    }

    #[test]
    fn header_authorization_env_promotes_to_env() {
        let mut a = args("x");
        a.http = Some("https://x".into());
        a.header = vec!["Authorization: Bearer ${GH_TOKEN}".into()];
        let (_, intent) = derive(a).expect("derive");
        assert!(matches!(intent, AuthIntent::Env(ref v) if v == "GH_TOKEN"));
    }

    #[test]
    fn header_non_authorization_kept_in_spec() {
        let mut a = args("x");
        a.http = Some("https://x".into());
        a.header = vec!["X-Api-Key: k1".into()];
        let (spec, intent) = derive(a).expect("derive");
        assert!(matches!(intent, AuthIntent::None));
        if let ServerSpec::Http { headers, .. } = spec {
            assert_eq!(headers.get("X-Api-Key").map(String::as_str), Some("k1"));
        }
    }

    #[test]
    fn stdio_with_bearer_is_rejected() {
        let mut a = args("x");
        a.command = vec!["echo".into(), "hi".into()];
        a.bearer = Some("abc".into());
        let err = derive(a).unwrap_err();
        assert!(err.to_string().contains("--http"));
    }

    #[test]
    fn header_missing_colon_is_rejected() {
        let mut a = args("x");
        a.http = Some("https://x".into());
        a.header = vec!["NoColonHere".into()];
        let err = derive(a).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("header"));
    }

    fn http(authorization: Option<&str>) -> ServerSpec {
        let mut headers = BTreeMap::new();
        if let Some(v) = authorization {
            headers.insert("Authorization".into(), v.into());
        }
        ServerSpec::Http {
            url: "https://example.test/mcp".into(),
            headers,
            auth: None,
        }
    }

    fn assert_no_auth_header(spec: &ServerSpec) {
        if let ServerSpec::Http { headers, .. } = spec {
            assert!(
                !headers
                    .keys()
                    .any(|k| k.eq_ignore_ascii_case("authorization")),
                "Authorization header survived: {headers:?}",
            );
        }
    }

    #[test]
    fn literal_bearer_strips_header() {
        let mut spec = http(Some("Bearer ghp_REALTOKEN"));
        let got = extract_bearer(&mut spec);
        assert_eq!(got, BearerSource::Literal("ghp_REALTOKEN".into()));
        assert_no_auth_header(&spec);
    }

    #[test]
    fn lowercase_bearer_recognised() {
        let mut spec = http(Some("bearer abc"));
        assert_eq!(
            extract_bearer(&mut spec),
            BearerSource::Literal("abc".into())
        );
    }

    #[test]
    fn header_name_case_insensitive() {
        let mut headers = BTreeMap::new();
        headers.insert("authorization".to_string(), "Bearer t".to_string());
        let mut spec = ServerSpec::Http {
            url: "https://x".into(),
            headers,
            auth: None,
        };
        assert_eq!(extract_bearer(&mut spec), BearerSource::Literal("t".into()));
        assert_no_auth_header(&spec);
    }

    #[test]
    fn braced_env_ref_becomes_bearer_env() {
        let mut spec = http(Some("Bearer ${GH_TOKEN}"));
        assert_eq!(
            extract_bearer(&mut spec),
            BearerSource::Env("GH_TOKEN".into())
        );
        let ServerSpec::Http { auth, .. } = &spec else {
            unreachable!()
        };
        assert!(matches!(auth, Some(AuthSpec::BearerEnv { env }) if env == "GH_TOKEN"));
        assert_no_auth_header(&spec);
    }

    #[test]
    fn unbraced_env_ref_becomes_bearer_env() {
        let mut spec = http(Some("Bearer $GH_TOKEN"));
        assert_eq!(
            extract_bearer(&mut spec),
            BearerSource::Env("GH_TOKEN".into())
        );
    }

    #[test]
    fn non_bearer_scheme_preserved() {
        let mut spec = http(Some("Basic dXNlcjpwYXNz"));
        assert_eq!(extract_bearer(&mut spec), BearerSource::None);
        let ServerSpec::Http { headers, .. } = &spec else {
            unreachable!()
        };
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Basic dXNlcjpwYXNz"),
        );
    }

    #[test]
    fn missing_header_is_no_op() {
        let mut spec = http(None);
        assert_eq!(extract_bearer(&mut spec), BearerSource::None);
        assert_no_auth_header(&spec);
    }

    #[test]
    fn stdio_spec_short_circuits() {
        let mut spec = ServerSpec::Stdio {
            command: "npx".into(),
            args: vec![],
            env: BTreeMap::new(),
        };
        assert_eq!(extract_bearer(&mut spec), BearerSource::None);
    }
}
