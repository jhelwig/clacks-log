//! Public certificate metadata and checked private signing credentials.

mod metadata;

use clacks_model::service::lotw::PublicSigningIdentity;
use time::{Date, OffsetDateTime};

/// Public issued-certificate metadata for display and signing eligibility.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificateMetadata {
    /// Certificate station callsign and DXCC entity.
    pub identity: PublicSigningIdentity,
    /// Complete encoded issuer name.
    pub issuer_der: Vec<u8>,
    /// Complete unsigned serial bytes, without integer narrowing.
    pub serial: Vec<u8>,
    /// Certificate validity beginning.
    pub not_before: OffsetDateTime,
    /// Certificate validity ending.
    pub not_after: OffsetDateTime,
    /// First covered QSO date, inclusive.
    pub qso_not_before: Date,
    /// Last covered QSO date, inclusive.
    pub qso_not_after: Date,
}

/// Interpret issued station metadata without asserting issuer trust or key possession.
///
/// Use this for stored public certificates. Rejects malformed, incomplete or
/// ambiguous station metadata and unsupported keys; performs no chain verification.
pub fn inspect_certificate(der: &[u8]) -> anyhow::Result<CertificateMetadata> {
    metadata::inspect(der)
}
