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

/// One complete successful key winner, restored through native validation.
#[derive(sqlx::FromRow)]
pub(crate) struct SnapshotRow {
    /// Canonical normalized request key.
    pub lookup_key: String,
    /// Provider that supplied the complete winner.
    pub provider: String,
    /// Original successful lookup Unix seconds.
    pub lookup_seconds: i64,
    /// Original successful lookup fractional nanoseconds.
    pub lookup_nanoseconds: i64,
    /// Original immutable request identity.
    pub lookup_id: String,
    /// Optional provider on-air name.
    pub name: Option<String>,
    /// Grid-area or explicit-point evidence form.
    pub position_kind: Option<String>,
    /// Projected grid area at supplied precision.
    pub locator: Option<String>,
    /// Projected point latitude in microdegrees.
    pub latitude: Option<i64>,
    /// Projected point longitude in microdegrees.
    pub longitude: Option<i64>,
    /// Point display locator precision.
    pub grid_length: Option<i64>,
    /// Station entity identifier.
    pub dxcc: Option<String>,
    /// Primary administrative subdivision.
    pub primary_subdivision: Option<String>,
    /// State-qualified secondary subdivision.
    pub secondary_subdivision: Option<String>,
    /// Checked CQ zone.
    pub cq_zone: Option<i64>,
    /// Checked ITU zone.
    pub itu_zone: Option<i64>,
    /// Directory locality without geographic inference.
    pub locality: Option<String>,
    /// Returned identity, separate from the requested key.
    pub returned_callsign: Option<String>,
    /// Original provider locator even when a point was projected.
    pub supplied_locator: Option<String>,
    /// Original reported latitude in microdegrees.
    pub supplied_latitude: Option<i64>,
    /// Original reported longitude in microdegrees.
    pub supplied_longitude: Option<i64>,
    /// Provider coordinate-origin classification.
    pub coordinate_source: Option<String>,
}

/// One separate provider reference association.
#[derive(sqlx::FromRow)]
pub(crate) struct ReferenceRow {
    /// Stable account group identity, independent of current definition availability.
    pub group_id: String,
    /// Optional qualified group identifier.
    pub identifier: Option<String>,
}
