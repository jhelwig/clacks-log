//! Relational review rows and field groups shared within the client store.

/// Raw SQL decoder columns for the saved reason for user resolution.
pub(crate) struct ReviewReasonRow {
    /// Review-reason discriminator.
    pub(crate) reason: String,
    /// Missing causal-evidence discriminator.
    pub(crate) unknown_reason: Option<String>,
    /// Kind of a missing referenced record.
    pub(crate) reference_kind: Option<String>,
    /// Native ID of a missing referenced record.
    pub(crate) reference_id: Option<String>,
    /// Actor excluded from the current account membership.
    pub(crate) actor_id: Option<String>,
}
