use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    #[default]
    Yaml,
    Json,
}

impl Format {
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

/// Filter a JSON value through a JMESPath expression. Used by callers that
/// want AWS-CLI `--query` semantics; returns the unchanged value when
/// `query` is `None`.
pub fn apply_query(value: Value, query: Option<&str>) -> Result<Value, Error> {
    let Some(expr) = query else {
        return Ok(value);
    };
    let compiled = jmespath::compile(expr).map_err(|e| Error::Query(format!("compile: {e}")))?;
    let v =
        jmespath::Variable::from_serializable(&value).map_err(|e| Error::Query(e.to_string()))?;
    let result = compiled
        .search(v)
        .map_err(|e| Error::Query(format!("search: {e}")))?;
    let s = serde_json::to_string(&*result).map_err(|e| Error::Query(e.to_string()))?;
    serde_json::from_str(&s).map_err(|e| Error::Query(e.to_string()))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error("jmespath {0}")]
    Query(String),
}
