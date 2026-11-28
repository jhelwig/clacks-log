//! Explicit, reproducible signing-eligibility decisions.

use super::CertificateMetadata;
use clacks_model::service::lotw::PublicSigningIdentity;
use jiff::{Timestamp, civil::Date, tz::Offset};

/// Current adapter-owned availability of a certificate's signing key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialState {
    /// The credential is available for eligibility evaluation.
    Available,
    /// An explicit management action excluded the credential from signing.
    Superseded,
    /// The key is no longer available, while public metadata may remain.
    Removed,
}

/// A specific condition preventing signing with the selected credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EligibilityIssue {
    /// The saved own-station callsign differs from the certificate identity.
    CallsignMismatch,
    /// The saved signing entity differs from the certificate entity.
    EntityMismatch,
    /// The contact precedes the certificate's inclusive QSO coverage.
    BeforeCoverage,
    /// The contact follows the certificate's inclusive QSO coverage.
    AfterCoverage,
    /// The certificate's final eligible UTC day has passed.
    Expired,
    /// The credential has been explicitly superseded.
    Superseded,
    /// The key was removed or otherwise cannot be used.
    KeyUnavailable,
}

/// Explain why a selected credential cannot sign the prepared identity and date.
///
/// Call during routing or before signing, supplying current state and evaluation
/// time. Returns every applicable reason in declaration order; an empty result
/// means locally eligible, not accepted by LoTW. Expiry uses UTC days, while QSO
/// coverage is inclusive. No issuer cache, hidden clock or routing preference is used.
pub fn eligibility(
    certificate: &CertificateMetadata,
    identity: &PublicSigningIdentity,
    qso_date: Date,
    state: CredentialState,
    now: Timestamp,
) -> Vec<EligibilityIssue> {
    let mut issues = Vec::new();
    if certificate.identity.callsign != identity.callsign {
        issues.push(EligibilityIssue::CallsignMismatch);
    }
    if certificate.identity.dxcc != identity.dxcc {
        issues.push(EligibilityIssue::EntityMismatch);
    }
    if qso_date < certificate.qso_not_before {
        issues.push(EligibilityIssue::BeforeCoverage);
    }
    if qso_date > certificate.qso_not_after {
        issues.push(EligibilityIssue::AfterCoverage);
    }
    if Offset::UTC.to_datetime(now).date() > certificate.not_after.date() {
        issues.push(EligibilityIssue::Expired);
    }
    match state {
        CredentialState::Available => {}
        CredentialState::Superseded => issues.push(EligibilityIssue::Superseded),
        CredentialState::Removed => issues.push(EligibilityIssue::KeyUnavailable),
    }
    if !issues.is_empty() {
        tracing::debug!(event="certificate.eligibility.unavailable", reasons=?issues);
    }
    issues
}
