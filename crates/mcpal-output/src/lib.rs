use std::io::{self, IsTerminal, Write};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Human,
    Json,
    Jsonl,
    Yaml,
}

impl Format {
    pub fn resolve(explicit: Option<Format>) -> Self {
        explicit.unwrap_or_else(|| {
            if io::stdout().is_terminal() {
                Self::Human
            } else {
                Self::Jsonl
            }
        })
    }
}

pub fn emit_json<T: Serialize>(val: &T) -> Result<(), Error> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, val)?;
    out.write_all(b"\n")?;
    Ok(())
}

pub fn emit_jsonl<T: Serialize>(val: &T) -> Result<(), Error> {
    let mut out = io::stdout().lock();
    serde_json::to_writer(&mut out, val)?;
    out.write_all(b"\n")?;
    Ok(())
}

/// JSON pretty by default; JSONL when stdout is piped and Format::Jsonl was selected.
pub fn emit_one<T: Serialize>(format: Format, value: &T) -> Result<(), Error> {
    match format {
        Format::Jsonl => emit_jsonl(value),
        _ => emit_json(value),
    }
}

/// Render a homogeneous list: JSON array, JSONL stream, or comfy-table.
pub fn emit_list<T, F>(
    format: Format,
    items: &[T],
    headers: &[&str],
    mut row: F,
) -> Result<(), Error>
where
    T: Serialize,
    F: FnMut(&T) -> Vec<String>,
{
    match format {
        Format::Json => emit_json(&items),
        Format::Jsonl => {
            for i in items {
                emit_jsonl(i)?;
            }
            Ok(())
        }
        Format::Human | Format::Yaml => {
            let mut t = comfy_table::Table::new();
            t.set_header(headers.to_vec());
            for i in items {
                t.add_row(row(i));
            }
            println!("{t}");
            Ok(())
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub use comfy_table;
pub use owo_colors;
