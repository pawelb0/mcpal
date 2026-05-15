use std::io::{IsTerminal, Read, Write};

use anyhow::{Context, Result, bail};
use mcpal_core::ServerSpec;
use mcpal_output::emit_one;
use serde_json::{Value, json};

use crate::cli::AuthAction;
use crate::keyring::{self, Kind};
use crate::oauth;
use crate::resolver::resolve;
use crate::runtime::Ctx;

pub async fn run(action: AuthAction, ctx: &Ctx) -> Result<()> {
    match action {
        AuthAction::Login {
            reference,
            bearer,
            oauth,
            url,
            no_browser,
        } => {
            login(
                &reference,
                bearer.as_deref(),
                oauth,
                url.as_deref(),
                no_browser,
                ctx,
            )
            .await
        }
        AuthAction::Logout { reference } => logout(&reference, ctx),
        AuthAction::Status { reference } => status(reference.as_deref(), ctx),
        AuthAction::Refresh { reference, url } => refresh(&reference, url.as_deref(), ctx).await,
    }
}

async fn login(
    reference: &str,
    bearer: Option<&str>,
    oauth_flag: bool,
    url: Option<&str>,
    no_browser: bool,
    ctx: &Ctx,
) -> Result<()> {
    if oauth_flag {
        let url = resolve_oauth_url(reference, url, ctx)?;
        oauth::login(reference, &url, !no_browser).await?;
        return report(
            ctx,
            json!({"ok": true, "ref": reference, "action": "login", "method": "oauth"}),
            || println!("authorized '{reference}' via OAuth"),
        );
    }

    let token = match bearer {
        Some("-") => read_stdin_token()?,
        Some(t) => t.to_string(),
        None => prompt_token()?,
    };
    if token.is_empty() {
        bail!("empty token");
    }
    keyring::put(reference, Kind::Bearer, &token)?;
    report(
        ctx,
        json!({"ok": true, "ref": reference, "action": "login", "method": "bearer"}),
        || println!("stored bearer for '{reference}'"),
    )
}

fn logout(reference: &str, ctx: &Ctx) -> Result<()> {
    keyring::delete(reference, Kind::Bearer)?;
    keyring::delete(reference, Kind::Oauth)?;
    report(
        ctx,
        json!({"ok": true, "ref": reference, "action": "logout"}),
        || println!("forgot credentials for '{reference}'"),
    )
}

fn status(reference: Option<&str>, ctx: &Ctx) -> Result<()> {
    let Some(reference) = reference else {
        bail!("pass a reference; listing all stored tokens isn't supported yet");
    };
    let bearer = keyring::get(reference, Kind::Bearer).is_some();
    let oauth_present = keyring::get(reference, Kind::Oauth).is_some();
    report(
        ctx,
        json!({
            "ref": reference,
            "bearer": bearer,
            "oauth": oauth_present,
        }),
        || {
            let kinds = match (bearer, oauth_present) {
                (false, false) => "no credentials",
                (true, false) => "bearer",
                (false, true) => "oauth",
                (true, true) => "bearer + oauth",
            };
            println!("{reference}: {kinds}");
        },
    )
}

async fn refresh(reference: &str, url: Option<&str>, ctx: &Ctx) -> Result<()> {
    let url = resolve_oauth_url(reference, url, ctx)?;
    oauth::refresh(reference, &url).await?;
    report(
        ctx,
        json!({"ok": true, "ref": reference, "action": "refresh"}),
        || println!("refreshed access token for '{reference}'"),
    )
}

/// Resolve the URL for an OAuth flow: explicit `--url` wins, then the resolved
/// alias's HTTP URL via the standard ref-resolver (so `cursor:linear`,
/// `mcpal server add`-defined names, and bare URLs all work).
fn resolve_oauth_url(reference: &str, override_url: Option<&str>, ctx: &Ctx) -> Result<String> {
    if let Some(u) = override_url {
        return Ok(u.to_string());
    }
    match resolve(reference, ctx)?.spec {
        ServerSpec::Http { url, .. } => Ok(url),
        ServerSpec::Stdio { .. } => {
            bail!("'{reference}' is a stdio server; OAuth only applies to HTTP")
        }
    }
}

fn report(ctx: &Ctx, payload: Value, _human: impl FnOnce()) -> Result<()> {
    emit_one(ctx.format, &payload).map_err(Into::into)
}

fn read_stdin_token() -> Result<String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("read stdin")?;
    Ok(buf.trim().to_string())
}

fn prompt_token() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        bail!("--bearer or piped stdin required when not on a TTY");
    }
    let mut out = std::io::stderr();
    write!(out, "bearer token: ").ok();
    out.flush().ok();
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).context("read line")?;
    Ok(buf.trim().to_string())
}
