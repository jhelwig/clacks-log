//! Identity information passed from protocol verification to account admission.

/// External identity claims after successful provider verification.
pub(crate) struct VerifiedIdentity {
    /// Exact provider issuer, without case folding.
    pub issuer: String,
    /// Exact provider subject, independent of display claims.
    pub subject: String,
    /// Optional display label; never used to infer ownership.
    pub display_name: Option<String>,
}
