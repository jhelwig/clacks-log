//! Caller-visible artifact and lookup failure categories.

use std::path::PathBuf;

/// A failed bundle read or geographic query; failures never mean uncovered geography.
#[derive(Debug, thiserror::Error)]
pub enum GeographyError {
    /// The bundle uses a format this reader cannot interpret.
    #[error("unsupported geographic bundle schema {0}")]
    UnsupportedSchema(u32),

    /// A bundle violates the declared format or integrity contract.
    #[error("invalid geographic artifact: {message}")]
    InvalidArtifact {
        /// The failed invariant and its relevant artifact identity.
        message: String,
    },

    /// Query geometry is outside the native geographic domain.
    #[error("invalid geographic query: {message}")]
    InvalidQuery {
        /// The invalid bound or coordinate condition.
        message: String,
    },

    /// A numerical operation cannot preserve a known topological decision.
    #[error("geographic numerical failure: {message}")]
    NumericalFailure {
        /// The operation whose result could not be established.
        message: String,
    },

    /// A required artifact file could not be read.
    #[error("read geographic artifact {}: {source}", path.display())]
    Read {
        /// The requested artifact file.
        path: PathBuf,
        /// The underlying filesystem error.
        #[source]
        source: std::io::Error,
    },

    /// A required artifact structure could not be decoded.
    #[error("decode geographic artifact {}: {source}", path.display())]
    Decode {
        /// The artifact being decoded.
        path: PathBuf,
        /// The decoding error, with its original causes.
        #[source]
        source: anyhow::Error,
    },
}

/// Construct a contextual semantic-format error.
pub(crate) fn invalid(message: impl Into<String>) -> GeographyError {
    GeographyError::InvalidArtifact {
        message: message.into(),
    }
}
