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
