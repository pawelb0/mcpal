use std::io::{IsTerminal, Read, Write};

use anyhow::{Context, Result, bail};
use mcpal_core::ServerSpec;
use serde_json::json;

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
        AuthAction::Logout { reference } => {
            keyring::delete(&reference, Kind::Bearer)?;
            keyring::delete(&reference, Kind::Oauth)?;
            ctx.render_one(&json!({"ok": true, "ref": reference, "action": "logout"}))?;
            Ok(())
        }
        AuthAction::Status { reference } => {
            let r = reference.ok_or_else(|| {
                anyhow::anyhow!("pass a reference; listing all stored tokens isn't supported yet")
            })?;
            ctx.render_one(&json!({
                "ref": r,
                "bearer": keyring::get(&r, Kind::Bearer).is_some(),
                "oauth": keyring::get(&r, Kind::Oauth).is_some(),
            }))?;
            Ok(())
        }
        AuthAction::Refresh { reference, url } => {
            let u = http_url(&reference, url.as_deref(), ctx)?;
            oauth::refresh(&reference, &u).await?;
            ctx.render_one(&json!({"ok": true, "ref": reference, "action": "refresh"}))?;
            Ok(())
        }
    }
}

pub(crate) async fn oauth_login_inline(
    reference: &str,
    override_url: Option<&str>,
    no_browser: bool,
    ctx: &Ctx,
) -> Result<()> {
    let url = http_url(reference, override_url, ctx)?;
    oauth::login(reference, &url, !no_browser).await?;
    Ok(())
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
        oauth_login_inline(reference, url, no_browser, ctx).await?;
        ctx.render_one(&json!({"ok": true, "ref": reference, "method": "oauth"}))?;
        return Ok(());
    }
    let token = match bearer {
        Some("-") => read_stdin()?,
        Some(t) => t.into(),
        None => prompt()?,
    };
    if token.is_empty() {
        bail!("empty token");
    }
    keyring::put(reference, Kind::Bearer, &token)?;
    ctx.render_one(&json!({"ok": true, "ref": reference, "method": "bearer"}))?;
    Ok(())
}

fn http_url(reference: &str, override_url: Option<&str>, ctx: &Ctx) -> Result<String> {
    if let Some(u) = override_url {
        return Ok(u.into());
    }
    match resolve(reference, ctx)?.spec {
        ServerSpec::Http { url, .. } => Ok(url),
        ServerSpec::Stdio { .. } => bail!("'{reference}' is a stdio server; OAuth needs HTTP"),
    }
}

pub(crate) fn read_stdin() -> Result<String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("read stdin")?;
    Ok(buf.trim().into())
}

fn prompt() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        bail!("--bearer or piped stdin required when not on a TTY");
    }
    write!(std::io::stderr(), "bearer token: ").ok();
    std::io::stderr().flush().ok();
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).context("read line")?;
    Ok(buf.trim().into())
}
