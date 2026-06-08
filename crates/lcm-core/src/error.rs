//! Error type shared across the core library.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("certificate parse error: {0}")]
    CertParse(String),

    #[error("no certificate found in input")]
    NoCert,

    #[error("unsupported distribution: {0}")]
    UnsupportedDistro(String),

    #[error("invalid name {0:?}: must contain at least one of [A-Za-z0-9._-]")]
    InvalidName(String),

    #[error("command {cmd} failed: {reason}")]
    Command { cmd: String, reason: String },

    #[error("unknown service: {0}")]
    UnknownService(String),

    #[error("a private key is required but was not provided")]
    MissingKey,

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
