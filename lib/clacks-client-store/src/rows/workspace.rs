//! Relational workspace rows and field groups shared within the client store.

/// Stored account identity and nullable enrollment fields.
pub(crate) struct EnrollmentRow {
    /// Owning native account.
    pub(crate) account: String,
    /// Enrolled server endpoint.
    pub(crate) server: Option<String>,
    /// Authorized client actor ID.
    pub(crate) actor: Option<String>,
    /// Reference to protected credential storage.
    pub(crate) credential_reference: Option<String>,
}
