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

/// A local archive cannot supply one checked station credential.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The supported export format requires password-based archive integrity.
    #[error("certificate archive has no integrity MAC")]
    MissingMac,
    /// The declared integrity derivation count is malformed rather than one round.
    #[error("certificate archive integrity iterations must be positive")]
    InvalidMacIterations,
    /// The selected archive decoder cannot represent this supplied password.
    #[error("archive password contains unsupported non-BMP characters")]
    UnsupportedPassword,
    /// No issued station certificate and private key are available.
    #[error("archive does not contain a station certificate and private key")]
    MissingMaterial,
    /// Distinct station/key candidates require selection outside this importer.
    #[error("archive contains ambiguous station certificate or key material")]
    AmbiguousMaterial,
    /// Actual key components disagree regardless of archive labels.
    #[error("archive private key does not match the station certificate")]
    KeyMismatch,
    /// RSA integer encodings exceed the supported validation work bound.
    #[error("RSA key exceeds the supported 4096-bit size")]
    KeyTooLarge,
}
