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

/// Prepared values or definitions cannot be represented by the signing protocol.
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    /// A signature definition is malformed, incompatible or unsupported.
    #[error("invalid signature definition: {reason}")]
    Definition {
        /// Public configuration context explaining the failure.
        reason: String,
    },
    /// A prepared field is absent or cannot be encoded without changing meaning.
    #[error("invalid signing field {field}: {reason}")]
    Field {
        /// Protocol field name, without its supplied value.
        field: String,
        /// Explanation that excludes supplied QSO or secret values.
        reason: &'static str,
    },
    /// Retained identity and prepared fields do not describe the same submission.
    #[error("prepared submission, external key and signing credential disagree")]
    IdentityMismatch,
    /// No contact exists to construct a signed batch.
    #[error("cannot construct an empty signed batch")]
    EmptyBatch,
    /// Local eligibility checks excluded an explicitly supplied item.
    #[error("batch item {index} is ineligible: {issues:?}")]
    Ineligible {
        /// Zero-based item position in the supplied batch.
        index: usize,
        /// All local eligibility failures for this item.
        issues: Vec<crate::certificate::eligibility::EligibilityIssue>,
    },
}
