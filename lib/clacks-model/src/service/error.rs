//! Contextual diagnostics for malformed service values and conversion limits.

/// Invalid service input, distinct from ordinary publication readiness.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// A service namespace or input violates its invariants.
    #[error("invalid service input: {detail}")]
    Invalid {
        /// Identity, field and violated constraint.
        detail: String,
    },
}
