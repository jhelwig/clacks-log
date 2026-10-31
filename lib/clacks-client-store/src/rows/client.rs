//! Relational client rows and field groups shared within the client store.

/// Relational fields distinguishing accepted, absent and unavailable shared knowledge.
pub(crate) struct ObservationRow {
    /// Stored observation discriminator.
    pub(crate) variant: String,
    /// Accepted envelope for an accepted observation.
    pub(crate) envelope_id: Option<i64>,
    /// Completed coverage certifying absence.
    pub(crate) checkpoint_id: Option<i64>,
    /// Missing-evidence discriminator for unavailable knowledge.
    pub(crate) unknown_reason: Option<String>,
}

/// Relational fields preserving an original value and its comparison evidence.
pub(crate) struct OriginalBasisRow {
    /// Stored original-basis discriminator.
    pub(crate) variant: String,
    /// Original accepted envelope.
    pub(crate) envelope_id: Option<i64>,
    /// Certified shared absence, when available for a known local creation.
    pub(crate) checkpoint_id: Option<i64>,
    /// Original native value retained after acceptance of this client's own change.
    pub(crate) payload_id: Option<i64>,
    /// Causal vector for the retained original value.
    pub(crate) vector_id: Option<i64>,
    /// Accepted stamp witnessing the retained original value.
    pub(crate) stamp_id: Option<i64>,
    /// Missing-evidence discriminator for an unavailable original.
    pub(crate) unknown_reason: Option<String>,
}

/// Qualified relational fields for an exact-version or certified-absence condition.
pub(crate) struct ConditionRow {
    /// Account owning the condition's target.
    pub(crate) account: String,
    /// Native record-kind discriminator for the target.
    pub(crate) kind: String,
    /// Native ID of the target record.
    pub(crate) record_id: String,
    /// Stored condition discriminator.
    pub(crate) variant: String,
    /// Accepted version required by an exact-version condition.
    pub(crate) stamp_id: Option<i64>,
    /// Completed coverage certifying an absence condition.
    pub(crate) checkpoint_id: Option<i64>,
}
