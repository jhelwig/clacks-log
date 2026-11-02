//! Relational issuer fields shared by persistence and authorization operations.

use time::OffsetDateTime;

/// Code lifecycle and optional encrypted replay receipt.
pub(crate) struct CodeRow {
    /// Account identity also used as the OAuth subject.
    pub account_id: String,
    /// Public application registration.
    pub client_id: String,
    /// Exact redirect granted at authorization time.
    pub redirect_uri: String,
    /// Whether the original request supplied its redirect explicitly.
    pub redirect_explicit: bool,
    /// S256 verifier challenge.
    pub code_challenge: String,
    /// Committed desktop signing-key thumbprint.
    pub key_thumbprint: String,
    /// Grant authorization instant.
    pub issued_at: OffsetDateTime,
    /// Code/receipt retention deadline.
    pub expires_at: OffsetDateTime,
    /// Library issued/consumed/replayed state.
    pub state: String,
    /// Complete encrypted issued-reference receipt, if issuance retained one.
    pub receipt: Option<String>,
    /// Granted scope set restored through the library parser.
    pub scope: String,
    /// Ordered granted resource indicators.
    pub resources: Vec<String>,
}

/// Access metadata joined to its single authoritative grant family.
pub(crate) struct AccessRow {
    /// Owning account.
    pub account_id: String,
    /// Grant-family identifier assigned by oauth-as.
    pub family_id: String,
    /// Public application registration.
    pub client_id: String,
    /// Bound proof key.
    pub key_thumbprint: String,
    /// Original approval time, not the current refresh time.
    pub grant_established_at: OffsetDateTime,
    /// Actual access issuance time.
    pub issued_at: OffsetDateTime,
    /// Actual access expiry.
    pub expires_at: OffsetDateTime,
    /// Family provisional/active/revoked state.
    pub family_state: String,
    /// Deadline retained while provisional, including after revocation.
    pub provisional_until: Option<OffsetDateTime>,
    /// Enrolled actor, absent before final code transition.
    pub actor_id: Option<String>,
    /// Actor closure time, if explicitly revoked.
    pub closed_at: Option<OffsetDateTime>,
    /// Granted scope set restored through the library parser.
    pub scope: String,
    /// Ordered resource indicators.
    pub resources: Vec<String>,
}

/// Refresh metadata joined to its single authoritative grant family.
pub(crate) struct RefreshRow {
    /// Owning account.
    pub account_id: String,
    /// Grant-family identifier assigned by oauth-as.
    pub family_id: String,
    /// Public application registration.
    pub client_id: String,
    /// Bound proof key.
    pub key_thumbprint: String,
    /// Original approval time, not the current refresh time.
    pub grant_established_at: OffsetDateTime,
    /// Optional refresh expiry.
    pub expires_at: Option<OffsetDateTime>,
    /// Dependency-owned refresh lifecycle discriminator.
    pub token_state: String,
    /// Family provisional/active/revoked state.
    pub family_state: String,
    /// Deadline retained while provisional, including after revocation.
    pub provisional_until: Option<OffsetDateTime>,
    /// Enrolled actor, absent before final code transition.
    pub actor_id: Option<String>,
    /// Actor closure time, if explicitly revoked.
    pub closed_at: Option<OffsetDateTime>,
    /// Granted scope set restored through the library parser.
    pub scope: String,
    /// Ordered resource indicators.
    pub resources: Vec<String>,
}

/// Grant eligibility serialized with account-owned issuance.
pub(crate) struct GrantRow {
    /// Owning account.
    pub account_id: String,
    /// Enrolled actor after activation.
    pub actor_id: Option<String>,
    /// Public application registration.
    pub client_id: String,
    /// Bound proof key.
    pub key_thumbprint: String,
    /// Original authorization time.
    pub grant_established_at: OffsetDateTime,
    /// Provisional, active or revoked state.
    pub state: String,
    /// Abandoned provisional grant deadline.
    pub provisional_until: Option<OffsetDateTime>,
}
