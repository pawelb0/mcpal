//! Core MCP client primitives shared across mcpal crates.
//!
//! Wraps [`rmcp`] with the small surface our CLI needs: a `ServerSpec` enum
//! that maps directly to TOML config, an `Error` taxonomy aligned with CLI
//! exit codes, and `connect()` to start a session.

mod client;
mod error;
mod spec;

pub use client::{Client, connect};
pub use error::{Error, Result};
pub use spec::{AuthSpec, ServerSpec};

pub use rmcp;
