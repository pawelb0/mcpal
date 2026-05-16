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

pub fn emit_one<T: Serialize>(format: Format, value: &T) -> Result<(), Error> {
    let mut out = io::stdout().lock();
    match format {
        Format::Json => {
            serde_json::to_writer_pretty(&mut out, value)?;
            out.write_all(b"\n")?;
        }
        Format::Yaml => serde_yaml::to_writer(&mut out, value)?,
    }
    Ok(())
}

pub fn emit_list<T: Serialize>(format: Format, items: &[T]) -> Result<(), Error> {
    emit_one(format, &items)
}

pub fn apply_query(value: Value, query: Option<&str>) -> Result<Value, Error> {
    let Some(expr) = query else { return Ok(value) };
    let compiled = jmespath::compile(expr).map_err(|e| Error::Query(format!("compile: {e}")))?;
    let v =
        jmespath::Variable::from_serializable(&value).map_err(|e| Error::Query(e.to_string()))?;
    let r = compiled.search(v).map_err(|e| Error::Query(format!("search: {e}")))?;
    let s = serde_json::to_string(&*r).map_err(|e| Error::Query(e.to_string()))?;
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
