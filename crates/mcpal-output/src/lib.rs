//! Output utilities for mcpal: format selection, JSON/JSONL emit, table builder.
//!
//! Per-noun rendering lives in `mcpal-cli`. This crate stays clap-free so it
//! can be reused by future TUI/daemon crates.

use std::io::{self, IsTerminal, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Human,
    Json,
    Jsonl,
    Yaml,
}

impl Format {
    /// Resolve `--output` against TTY: defaults to Human on a terminal, JSONL when piped.
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

/// Pretty JSON, single document, trailing newline.
pub fn emit_json<T: serde::Serialize>(val: &T) -> Result<(), Error> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, val)?;
    out.write_all(b"\n")?;
    Ok(())
}

/// One JSON record per line, no pretty-printing.
pub fn emit_jsonl<T: serde::Serialize>(val: &T) -> Result<(), Error> {
    let mut out = io::stdout().lock();
    serde_json::to_writer(&mut out, val)?;
    out.write_all(b"\n")?;
    Ok(())
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
