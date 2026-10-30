//! Named encrypted-record projections shared by persistence and maintenance.

/// Encrypted code receipt with the immutable approval context needed for migration.
pub(crate) struct ReceiptEnvelopeRow {
    /// Owning account.
    pub account_id: String,
    /// One-way code identity, not the original code.
    pub code_digest: Vec<u8>,
    /// Immutable approval purpose; missing context is an error.
    pub purpose: Option<String>,
    /// Complete receipt selected through its generated account-key reference.
    pub receipt: Option<String>,
}
