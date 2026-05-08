use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("transport: {0:#}")]
    Transport(#[source] anyhow::Error),

    #[error("auth: {0:#}")]
    Auth(#[source] anyhow::Error),

    #[error("protocol: {0:#}")]
    Protocol(#[source] anyhow::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("{0} is not yet supported")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Stable exit code for this error category. Mirrors `mcpal-cli/src/exit.rs`.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Transport(_) | Self::Unsupported(_) => 3,
            Self::Auth(_) => 4,
            Self::Protocol(_) => 5,
            Self::NotFound(_) => 7,
        }
    }
}
