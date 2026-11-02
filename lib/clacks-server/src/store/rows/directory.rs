//! Named directory configuration and encrypted-material row fields.

/// Public persisted account-level selection and stale-write revision.
pub(crate) struct ConfigurationRow {
    /// Committed semantic configuration version.
    pub revision: i64,
    /// Provider label, absent when disabled.
    pub selected_provider: Option<String>,
}

/// Complete encrypted material and its independently stored binding.
pub(crate) struct MaterialRow {
    /// Expected owner, qualified in every query.
    pub account_id: String,
    /// Expected concrete directory purpose.
    pub provider: String,
    /// Stable material identity until explicit removal.
    pub material_id: String,
    /// Semantic credential version.
    pub material_revision: i64,
    /// One complete authenticated envelope.
    pub encrypted_envelope: String,
}
