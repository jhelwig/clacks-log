//! Transient operating and reusable-record editing, separate from durable state.

pub(crate) mod editor;
pub(crate) mod equipment;
pub(crate) mod position;
pub(crate) mod power;
pub(crate) mod station;

#[cfg(test)]
mod tests;

/// Field-local validation feedback retained next to an editable form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FieldError {
    /// Human-readable field label.
    pub field: String,
    /// Actionable feedback for the user; not operational telemetry.
    pub message: String,
}

/// Attach native validation feedback to its editable field.
fn issue(field: &str, message: impl std::fmt::Display) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.to_string(),
    }
}

/// Preserve nonblank native text while treating blank optional input as absent.
fn optional(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_owned())
}
