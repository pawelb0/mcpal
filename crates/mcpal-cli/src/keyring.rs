use anyhow::{Context, Result};

const SERVICE: &str = "mcpal";

fn entry(reference: &str) -> Result<keyring::Entry> {
    let account = format!("bearer:{reference}");
    keyring::Entry::new(SERVICE, &account).with_context(|| format!("keyring entry for {account}"))
}

pub fn put_bearer(reference: &str, token: &str) -> Result<()> {
    entry(reference)?
        .set_password(token)
        .with_context(|| format!("store bearer for {reference}"))
}

pub fn get_bearer(reference: &str) -> Option<String> {
    entry(reference).ok()?.get_password().ok()
}

pub fn delete_bearer(reference: &str) -> Result<()> {
    let e = entry(reference)?;
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("delete bearer for {reference}")),
    }
}
