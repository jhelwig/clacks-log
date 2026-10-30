//! Named encrypted-record projections shared by persistence and maintenance.

/// Stored service credential, including its logical identity and optional envelope.
pub(crate) struct CredentialRow {
    /// Account owning the credential.
    pub account_id: String,
    /// Stable logical credential identity.
    pub credential_id: String,
    /// Immutable service purpose.
    pub purpose: String,
    /// Revision of provider material, independent of encryption maintenance.
    pub material_revision: i64,
    /// Present material or a retained removal.
    pub state: String,
    /// Owning codec version for present material.
    pub payload_format: Option<i32>,
    /// Exact account key needed for present material.
    pub key_version: Option<i64>,
    /// Authentication nonce for present material.
    pub nonce: Option<Vec<u8>>,
    /// Protected material and authentication tag.
    pub ciphertext: Option<Vec<u8>>,
    /// Logical creation instant.
    pub created_at: time::OffsetDateTime,
    /// Last explicit material change, unaffected by maintenance.
    pub changed_at: time::OffsetDateTime,
}
