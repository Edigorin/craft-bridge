//! Error type for the bridge. Input is hostile, so every failure is a value:
//! nothing here may panic.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {what}: {source}")]
    Io {
        what: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{what} is not valid JSON: {source}")]
    Json {
        what: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("{what} is over the limit: {actual} bytes, limit {limit} bytes")]
    LimitExceeded {
        what: String,
        limit: u64,
        actual: u64,
    },

    #[error("invalid artifact: {0}")]
    InvalidArtifact(String),

    #[error("unsupported replay profile {found} (this build understands {supported})")]
    UnsupportedProfile { found: String, supported: String },

    #[error("invalid recipe: {0}")]
    InvalidRecipe(String),

    #[error("invalid warnings sidecar: {0}")]
    InvalidWarnings(String),

    #[error("invalid STEP geometry: {0}")]
    InvalidStep(String),

    #[error("cannot write {path}: {reason}")]
    Output { path: PathBuf, reason: String },
}

impl Error {
    pub fn io(what: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io {
            what: what.into(),
            source,
        }
    }

    pub fn json(what: impl Into<String>, source: serde_json::Error) -> Self {
        Error::Json {
            what: what.into(),
            source,
        }
    }

    pub fn limit(what: impl Into<String>, limit: u64, actual: u64) -> Self {
        Error::LimitExceeded {
            what: what.into(),
            limit,
            actual,
        }
    }

    pub fn output(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Error::Output {
            path: path.into(),
            reason: reason.into(),
        }
    }
}
