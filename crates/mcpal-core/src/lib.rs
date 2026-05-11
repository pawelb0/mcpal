mod client;
mod error;
mod spec;

pub use client::{Client, connect};
pub use error::{Error, Result};
pub use spec::{AuthSpec, ServerSpec};

pub use rmcp;
