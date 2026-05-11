use anyhow::{Context, Result};

const SERVICE: &str = "mcpal";

fn entry(account: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, account).with_context(|| format!("keyring entry for {account}"))
}

fn account_bearer(reference: &str) -> String {
    format!("bearer:{reference}")
}

fn account_oauth(reference: &str) -> String {
    format!("oauth:{reference}")
}

pub fn put_bearer(reference: &str, token: &str) -> Result<()> {
    entry(&account_bearer(reference))?
        .set_password(token)
        .with_context(|| format!("store bearer for {reference}"))
}

pub fn get_bearer(reference: &str) -> Option<String> {
    entry(&account_bearer(reference)).ok()?.get_password().ok()
}

pub fn delete_bearer(reference: &str) -> Result<()> {
    delete_account(&account_bearer(reference), reference, "bearer")
}

pub fn put_oauth_blob(reference: &str, json: &str) -> Result<()> {
    entry(&account_oauth(reference))?
        .set_password(json)
        .with_context(|| format!("store oauth creds for {reference}"))
}

pub fn get_oauth_blob(reference: &str) -> Option<String> {
    entry(&account_oauth(reference)).ok()?.get_password().ok()
}

pub fn delete_oauth_blob(reference: &str) -> Result<()> {
    delete_account(&account_oauth(reference), reference, "oauth")
}

fn delete_account(account: &str, reference: &str, what: &str) -> Result<()> {
    let e = entry(account)?;
    match e.delete_credential() {
        // Idempotent: deleting a non-existent entry is a no-op.
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("delete {what} for {reference}")),
    }
}
