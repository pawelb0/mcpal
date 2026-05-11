use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("rmcp service: {0}")]
    Service(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("{0} is not yet supported")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Io(_) | Self::Unsupported(_) => 3,
            Self::Service(_) => 5,
            Self::NotFound(_) => 7,
        }
    }
}
