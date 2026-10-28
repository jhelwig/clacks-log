//! Typed operator expressions for local history; execution belongs to adapters.

pub mod completion;
pub mod diagnostic;
pub mod field;
mod syntax;
pub mod value;

use crate::recognition::RecognitionCatalog;
use diagnostic::QueryDiagnostic;
use field::Field;
use value::Value;

/// A typed restriction on one operator-facing field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Predicate {
    /// Public field identity, independent of storage layout.
    pub field: Field,
    /// Typed matching rule or presence test.
    pub value: Value,
}

/// Boolean query semantics with ordinary two-valued negation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Query {
    /// No expression restriction; the selected view still applies.
    All,
    /// One typed field restriction.
    Predicate(Predicate),
    /// Negation, including missing values when the positive predicate is false.
    Not(Box<Query>),
    /// Both restrictions must match, including repeated fields.
    And(Box<Query>, Box<Query>),
    /// Either restriction may match.
    Or(Box<Query>, Box<Query>),
}

/// Validate expression text for application, preserving diagnostic source ranges.
///
/// Invalid or unfinished input returns a diagnostic rather than an unrestricted
/// query. Callers retain the last applied expression on failure.
#[tracing::instrument(skip_all, name = "model.query.parse", level = "debug")]
pub fn parse(text: &str, catalog: &RecognitionCatalog) -> Result<Query, QueryDiagnostic> {
    match syntax::parse(text, catalog) {
        Ok(query) => Ok(query),
        Err(error) => {
            tracing::debug!(event = "query.rejected", span_start = error.span.start, span_end = error.span.end, reason = %error.message);
            Err(error)
        }
    }
}
