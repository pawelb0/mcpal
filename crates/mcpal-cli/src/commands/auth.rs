use std::io::{IsTerminal, Read, Write};

use anyhow::{Context, Result, bail};
use mcpal_output::{Format, emit_one};
use serde_json::json;

use crate::cli::AuthAction;
use crate::keyring;
use crate::runtime::Ctx;

pub fn run(action: AuthAction, ctx: &Ctx) -> Result<()> {
    match action {
        AuthAction::Login { reference, bearer } => login(&reference, bearer.as_deref(), ctx),
        AuthAction::Logout { reference } => logout(&reference, ctx),
        AuthAction::Status { reference } => status(reference.as_deref(), ctx),
    }
}

fn login(reference: &str, bearer: Option<&str>, ctx: &Ctx) -> Result<()> {
    let token = match bearer {
        Some("-") => read_stdin_token()?,
        Some(t) => t.to_string(),
        None => prompt_token()?,
    };
    if token.is_empty() {
        bail!("empty token");
    }
    keyring::put_bearer(reference, &token)?;
    match ctx.format {
        Format::Json | Format::Jsonl => emit_one(
            ctx.format,
            &json!({"ok": true, "ref": reference, "action": "login"}),
        )?,
        _ => println!("stored bearer for '{reference}'"),
    }
    Ok(())
}

fn logout(reference: &str, ctx: &Ctx) -> Result<()> {
    keyring::delete_bearer(reference)?;
    match ctx.format {
        Format::Json | Format::Jsonl => emit_one(
            ctx.format,
            &json!({"ok": true, "ref": reference, "action": "logout"}),
        )?,
        _ => println!("forgot bearer for '{reference}'"),
    }
    Ok(())
}

fn status(reference: Option<&str>, ctx: &Ctx) -> Result<()> {
    let Some(reference) = reference else {
        bail!("listing all stored tokens is not yet supported; pass a reference");
    };
    let present = keyring::get_bearer(reference).is_some();
    match ctx.format {
        Format::Json | Format::Jsonl => {
            emit_one(ctx.format, &json!({"ref": reference, "stored": present}))?;
        }
        _ => println!(
            "{reference}: {}",
            if present { "stored" } else { "no token" }
        ),
    }
    Ok(())
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
