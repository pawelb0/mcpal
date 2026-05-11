use anyhow::{Context, Result};

const SERVICE: &str = "mcpal";

#[derive(Clone, Copy)]
pub enum Kind {
    Bearer,
    Oauth,
}

impl Kind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Bearer => "bearer",
            Self::Oauth => "oauth",
        }
    }
}

fn entry(reference: &str, kind: Kind) -> Result<keyring::Entry> {
    let account = format!("{}:{reference}", kind.prefix());
    keyring::Entry::new(SERVICE, &account).with_context(|| format!("keyring entry for {account}"))
}

pub fn put(reference: &str, kind: Kind, value: &str) -> Result<()> {
    entry(reference, kind)?
        .set_password(value)
        .with_context(|| format!("store {} for {reference}", kind.prefix()))
}

pub fn get(reference: &str, kind: Kind) -> Option<String> {
    entry(reference, kind).ok()?.get_password().ok()
}

pub fn delete(reference: &str, kind: Kind) -> Result<()> {
    let e = entry(reference, kind)?;
    match e.delete_credential() {
        // Idempotent: deleting a non-existent entry is a no-op.
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("delete {} for {reference}", kind.prefix())),
    }
}
