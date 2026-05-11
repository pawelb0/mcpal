use std::io::{IsTerminal, Read, Write};

use anyhow::{Context, Result, bail};
use mcpal_core::ServerSpec;
use mcpal_output::{Format, emit_one};
use serde_json::{Value, json};

use crate::cli::AuthAction;
use crate::config::Config;
use crate::keyring;
use crate::oauth;
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
        let url = oauth_url_for(reference, url, &ctx.cfg)?;
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
    keyring::put_bearer(reference, &token)?;
    report(
        ctx,
        json!({"ok": true, "ref": reference, "action": "login", "method": "bearer"}),
        || println!("stored bearer for '{reference}'"),
    )
}

fn logout(reference: &str, ctx: &Ctx) -> Result<()> {
    keyring::delete_bearer(reference)?;
    keyring::delete_oauth_blob(reference)?;
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
    let bearer = keyring::get_bearer(reference).is_some();
    let oauth_present = keyring::get_oauth_blob(reference).is_some();
    report(
        ctx,
        json!({
            "ref": reference,
            "bearer": bearer,
            "oauth": oauth_present,
        }),
        || {
            let kinds = match (bearer, oauth_present) {
                (false, false) => "no credentials".to_string(),
                (true, false) => "bearer".to_string(),
                (false, true) => "oauth".to_string(),
                (true, true) => "bearer + oauth".to_string(),
            };
            println!("{reference}: {kinds}");
        },
    )
}

async fn refresh(reference: &str, url: Option<&str>, ctx: &Ctx) -> Result<()> {
    let url = oauth_url_for(reference, url, &ctx.cfg)?;
    oauth::refresh(reference, &url).await?;
    report(
        ctx,
        json!({"ok": true, "ref": reference, "action": "refresh"}),
        || println!("refreshed access token for '{reference}'"),
    )
}

fn oauth_url_for(reference: &str, override_url: Option<&str>, cfg: &Config) -> Result<String> {
    if let Some(u) = override_url {
        return Ok(u.to_string());
    }
    if let Some(ServerSpec::Http { url, .. }) = cfg.server.get(reference) {
        return Ok(url.clone());
    }
    bail!(
        "no URL for '{reference}'; pass --url or add it as an HTTP server first \
         (`mcpal server add {reference} --http <url>`)"
    )
}

fn report(ctx: &Ctx, payload: Value, human: impl FnOnce()) -> Result<()> {
    match ctx.format {
        Format::Json | Format::Jsonl => emit_one(ctx.format, &payload).map_err(Into::into),
        _ => {
            human();
            Ok(())
        }
    }
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
