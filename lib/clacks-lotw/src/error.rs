//! Local credential and signing validation failures.

/// A required certificate value cannot be interpreted unambiguously.
#[derive(Debug, thiserror::Error)]
#[error("invalid certificate field {field}: {reason}")]
pub struct CertificateError {
    /// Protocol field whose value could not be accepted.
    pub field: &'static str,
    /// Explanation without secret key or archive content.
    pub reason: &'static str,
}
