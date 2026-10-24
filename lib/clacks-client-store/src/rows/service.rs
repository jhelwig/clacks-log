//! Relational service rows and field groups shared within the client store.

/// Relational columns for a public destination descriptor.
pub(crate) struct DestinationRow {
    /// Native destination identity.
    pub(crate) destination_id: String,
    /// Provider collection discriminator.
    pub(crate) collection: String,
    /// LoTW account namespace.
    pub(crate) lotw_account: Option<String>,
    /// Known QRZ book identity.
    pub(crate) qrz_book: Option<String>,
    /// Stored display description.
    pub(crate) description: String,
}

/// Borrowed namespace columns for one public destination descriptor.
pub(crate) struct CollectionFields<'a> {
    /// Provider collection discriminator.
    pub(crate) collection: &'a str,
    /// LoTW account namespace.
    pub(crate) lotw_account: Option<&'a str>,
    /// Known QRZ book ID, absent for historical unknown books.
    pub(crate) qrz_book: Option<&'a str>,
}

/// Borrowed columns retaining an exact provider record identity.
pub(crate) struct ExternalKeyFields<'a> {
    /// Provider discriminator.
    pub(crate) service: &'static str,
    /// Canonical QRZ record ID.
    pub(crate) qrz_id: Option<&'a str>,
    /// LoTW own-station callsign.
    pub(crate) own_call: Option<&'a str>,
    /// LoTW contacted-station callsign.
    pub(crate) worked_call: Option<&'a str>,
    /// LoTW contact start in whole UTC seconds.
    pub(crate) start: Option<i64>,
    /// LoTW primary band identity.
    pub(crate) band: Option<&'a str>,
    /// LoTW receive-band identity.
    pub(crate) receive_band: Option<&'a str>,
    /// LoTW normalized mode identity.
    pub(crate) mode: Option<&'a str>,
}

/// Borrowed provider projection columns and owned field-map references.
pub(crate) struct ProjectionFields<'a> {
    /// Provider discriminator.
    pub(crate) service: &'static str,
    /// Historical LoTW mapping release.
    pub(crate) mapping_release: Option<&'a str>,
    /// Historical upstream mapping commit.
    pub(crate) mapping_commit: Option<&'a str>,
    /// Callsign used for signing.
    pub(crate) signing_call: Option<&'a str>,
    /// Entity used for signing.
    pub(crate) signing_dxcc: Option<&'a str>,
    /// Exact LoTW external-record key.
    pub(crate) external_id: Option<i64>,
    /// Frozen contact or QRZ field map.
    pub(crate) contact_map: i64,
    /// Frozen LoTW station field map.
    pub(crate) station_map: Option<i64>,
}

/// Columns distinguishing complete keys from incompletely identified observations.
pub(crate) struct RemoteIdentityFields<'a> {
    /// Complete or incomplete identity discriminator.
    pub(crate) identity: &'a str,
    /// Exact external-record key for a complete identity.
    pub(crate) external_id: Option<i64>,
    /// Provider discriminator for an incomplete identity.
    pub(crate) incomplete_service: Option<&'a str>,
    /// Available identity fields when the full key is unknown.
    pub(crate) available_map: Option<i64>,
}
