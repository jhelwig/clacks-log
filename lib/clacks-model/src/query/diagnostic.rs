//! Query diagnostics identify editable input without embedding private query text.

use std::ops::Range;

/// A query cannot yet be applied at this source range.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct QueryDiagnostic {
    /// Byte range to underline in the original expression.
    pub span: Range<usize>,
    /// Actionable explanation that excludes the supplied value.
    pub message: String,
}

impl QueryDiagnostic {
    /// Describe invalid input at a known byte range.
    pub(crate) fn new(span: Range<usize>, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}
