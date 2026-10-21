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

/// Reportable output conversion loss, preserving the original native value.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectionWarning {
    /// Output field affected.
    pub field: String,
    /// Human-readable conversion or omission explanation.
    pub detail: String,
}

/// A normalized projection together with nonfatal conversion warnings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionResult<T> {
    /// Semantic values to retain or transmit.
    pub value: T,
    /// Losses or omissions to report.
    pub warnings: Vec<ProjectionWarning>,
}

/// A service field that prevents publication without invalidating the native QSO.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessIssue {
    /// Field or namespace requiring remedy.
    pub field: String,
    /// Concrete offending value or missing requirement.
    pub reason: String,
}

/// Publication preparation result; readiness is distinct from corrupt input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionDecision<T> {
    /// Normalized values available for comparison and dispatch.
    Ready(ProjectionResult<T>),
    /// User/configuration remedy is needed for these fields.
    NotReady(Vec<ReadinessIssue>),
}
