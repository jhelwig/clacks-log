//! Relational references for copied operating facts and independent selected sources.

/// Account-qualified current facts and optional immutable source payload roots.
pub(crate) struct OperatingStateRow {
    /// Current owned operating snapshot.
    pub(crate) current_values: Option<i64>,
    /// Exact saved profile loaded into the station fields.
    pub(crate) loaded_profile: Option<i64>,
    /// Identity belonging to the profile baseline.
    pub(crate) profile_record: Option<String>,
    /// Exact saved configuration loaded into equipment fields.
    pub(crate) loaded_equipment: Option<i64>,
    /// Identity belonging to the equipment baseline.
    pub(crate) equipment_record: Option<String>,
}
