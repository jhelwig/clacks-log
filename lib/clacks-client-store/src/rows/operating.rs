//! Relational operating rows and field groups shared within the client store.

/// Stored references for current defaults and an optional complete loaded baseline.
pub(crate) struct OperatingStateRow {
    /// Current owned operating snapshot.
    pub(crate) current_values: Option<i64>,
    /// Copied loaded profile payload.
    pub(crate) loaded_profile: Option<i64>,
    /// Native ID of the copied profile.
    pub(crate) profile_record: Option<String>,
    /// Operating snapshot captured when the profile was loaded.
    pub(crate) loaded_values: Option<i64>,
}
