mod client;
mod error;
mod handler;
mod spec;

pub use client::{Client, connect};
pub use error::{Error, Result};
pub use handler::{Handler, HandlerOptions};
pub use spec::{AuthSpec, ServerSpec};

pub use rmcp;
