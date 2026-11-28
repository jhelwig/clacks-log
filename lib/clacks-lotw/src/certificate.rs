//! Public certificate metadata and checked private signing credentials.

mod archive;
mod metadata;

pub mod eligibility;

use clacks_model::service::lotw::PublicSigningIdentity;
use jiff::civil::{Date, DateTime};

/// Public issued-certificate metadata for display and signing eligibility.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificateMetadata {
    /// Certificate station callsign and DXCC entity.
    pub identity: PublicSigningIdentity,
    /// Complete encoded issuer name.
    pub issuer_der: Vec<u8>,
    /// Complete unsigned serial bytes, without integer narrowing.
    pub serial: Vec<u8>,
    /// Certificate validity beginning, expressed in UTC.
    pub not_before: DateTime,
    /// Certificate validity ending, expressed in UTC, including year 9999.
    pub not_after: DateTime,
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

/// An imported station certificate with a validated matching private signing key.
///
/// Keep this server-side. It exposes public metadata and certificate bytes, but
/// intentionally offers no raw-key export, cloning or secret-bearing formatting.
pub struct SigningCredential {
    /// Public interpretation retained independently of key use.
    metadata: CertificateMetadata,
    /// Original station certificate bytes for protocol emission.
    certificate_der: Vec<u8>,
    /// Digest-aware RSA key; private components are erased by the dependency on drop.
    pub(crate) signing_key: rsa::pkcs1v15::SigningKey<sha1::Sha1>,
}

impl SigningCredential {
    /// Borrow the issued certificate's public metadata for display or routing.
    pub fn metadata(&self) -> &CertificateMetadata {
        &self.metadata
    }

    /// Borrow the exact imported certificate encoding for signed output.
    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate_der
    }
}

/// Import one station credential without storing it or accepting its issuer chain.
///
/// Requires archive integrity, interpretable station metadata and an actual RSA
/// public-key match. Password/integrity failures remain ambiguous. Existing
/// credentials are unaffected; non-BMP passwords and ambiguous material fail.
#[tracing::instrument(skip_all, name = "lotw.certificate.import", level = "debug")]
pub fn import_pkcs12(archive: &[u8], password: &str) -> anyhow::Result<SigningCredential> {
    archive::import(archive, password)
}
