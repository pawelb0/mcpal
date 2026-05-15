use std::io::{self, Write};

use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    #[default]
    Yaml,
    Json,
}

impl Format {
    /// Honor `--output` if set, else default to YAML.
    pub fn resolve(explicit: Option<Format>) -> Self {
        explicit.unwrap_or(Self::Yaml)
    }
}

pub fn emit_json<T: Serialize>(val: &T) -> Result<(), Error> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, val)?;
    out.write_all(b"\n")?;
    Ok(())
}

pub fn emit_yaml<T: Serialize>(val: &T) -> Result<(), Error> {
    let mut out = io::stdout().lock();
    serde_yaml::to_writer(&mut out, val)?;
    Ok(())
}

pub fn emit_one<T: Serialize>(format: Format, value: &T) -> Result<(), Error> {
    match format {
        Format::Json => emit_json(value),
        Format::Yaml => emit_yaml(value),
    }
}

/// Render a list. `headers` and `row` are accepted for backward compatibility
/// but ignored — both YAML and JSON output the typed list directly.
pub fn emit_list<T, F>(format: Format, items: &[T], _headers: &[&str], _row: F) -> Result<(), Error>
where
    T: Serialize,
    F: FnMut(&T) -> Vec<String>,
{
    match format {
        Format::Json => emit_json(&items),
        Format::Yaml => emit_yaml(&items),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}
