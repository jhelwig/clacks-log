//! Determine whether upload and confirmation evidence applies to the current QSO.

use super::{
    adif::AdifFields,
    destination::Service,
    evidence::{
        ImportedAssertion, ImportedBaseline, ImportedClaim,
        observation::{ConfirmationStatus, Reported, StationReport},
        submission::{
            ConfirmationBasis, SubmissionAttempt, SubmissionOutcome, SubmissionProjection,
            SubmissionResult,
        },
    },
};
use crate::{effective::StationFacts, ids::QsoId, location::RecordedLocation};

/// Whether current service values have explicit publication coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationCoverage {
    /// A recognized imported assertion or explicit success covers current values.
    Covered,
    /// No positive evidence covers these values; normal scheduling still applies.
    Pending,
    /// Result support is missing, so its coverage cannot be established.
    NotEstablished,
}

/// Current basic confirmation applicability, independent of upload need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmationApplicability {
    /// Applicable positive evidence, without inferred exact-mode/award qualifications.
    Confirmed,
    /// Applicable explicit negative evidence.
    Unconfirmed,
    /// Available evidence cannot establish the changed contact's applicability.
    NotEstablished,
    /// Retained evidence describes a different contact identity.
    HistoricalOnly,
}

/// Read-only current values and retained scope-qualified publication evidence.
pub struct PublicationCoverageInput<'a> {
    /// Contact being evaluated.
    pub qso: QsoId,
    /// Service evaluated independently of routing or signing readiness.
    pub service: Service,
    /// Selected remote destination when known; no invented default is needed for imported claims.
    pub destination: Option<crate::ids::DestinationId>,
    /// Prepared current output when conversion/signing context is available.
    pub current: Option<&'a SubmissionProjection>,
    /// Current source-only values under the same import comparison rules.
    pub source_current: &'a ImportedBaseline,
    /// Recognized imported claims in this account.
    pub assertions: &'a [ImportedAssertion],
    /// Retained actual dispatches.
    pub attempts: &'a [SubmissionAttempt],
    /// Results with original scope and request correlation.
    pub results: &'a [SubmissionResult],
}

/// Compare current output with actual successes and trusted imported baselines.
#[tracing::instrument(
    skip_all,
    name = "model.publication.coverage",
    level = "debug",
    fields(
        qso_id = %input.qso,
    ),
)]
pub fn publication_coverage(
    input: &PublicationCoverageInput<'_>,
) -> anyhow::Result<PublicationCoverage> {
    let destination = input
        .destination
        .or_else(|| input.current.map(SubmissionProjection::destination));
    let service = input.service;
    anyhow::ensure!(
        input.source_current.service == service
            && input.current.is_none_or(
                |p| p.service() == service && destination.is_none_or(|d| d == p.destination())
            ),
        "QSO {} has incompatible publication comparison service/destination",
        input.qso
    );

    for assertion in input.assertions.iter().filter(|a| {
        a.qso == input.qso
            && a.service == service
            && a.claim == ImportedClaim::Uploaded
            && a.destination
                .is_none_or(|d| destination.is_none_or(|selected| d == selected))
    }) {
        let same = if service == Service::Lotw {
            assertion.baseline.fields == input.source_current.fields
                && assertion.baseline.own_location == input.source_current.own_location
                && assertion.baseline.confirmation.start == input.source_current.confirmation.start
        } else {
            match input.current {
                Some(SubmissionProjection::Qrz(v)) => assertion.baseline.fields == v.fields,
                None => {
                    assertion.baseline.fields == input.source_current.fields
                        && assertion.baseline.confirmation.start
                            == input.source_current.confirmation.start
                        && assertion.baseline.native_end == input.source_current.native_end
                }
                _ => false,
            }
        };
        if same {
            tracing::debug!(
                event = "publication.coverage_decided",
                ?service,
                destination_id = destination.map(tracing::field::display),
                coverage = ?PublicationCoverage::Covered,
                assertion_id = %assertion.id,
            );
            return Ok(PublicationCoverage::Covered);
        }
    }
    let mut missing = false;
    for result in input.results.iter().filter(|r| {
        destination.is_none_or(|d| r.destination == d) && r.outcome == SubmissionOutcome::Success
    }) {
        for id in result.scope.attempts() {
            let Some(attempt) = input.attempts.iter().find(|a| a.id == id) else {
                tracing::debug!(
                    event = "publication.support_missing",
                    reason = "attempt_unavailable",
                    result_id = %result.id,
                    attempt_id = %id,
                );
                missing = true;
                continue;
            };
            if let super::evidence::submission::ResultScope::UploadBatch { batch, .. } =
                &result.scope
                && attempt.batch != Some(*batch)
            {
                tracing::debug!(
                    event = "publication.support_missing",
                    reason = "batch_mismatch",
                    result_id = %result.id,
                    attempt_id = %attempt.id,
                );
                missing = true;
                continue;
            }
            if attempt.qso == input.qso && attempt.projection.service() == service {
                if let Some(current) = input.current {
                    if attempt.projection.same_payload(current) {
                        tracing::debug!(
                            event = "publication.coverage_decided",
                            ?service,
                            destination_id = destination.map(tracing::field::display),
                            coverage = ?PublicationCoverage::Covered,
                            result_id = %result.id,
                            attempt_id = %attempt.id,
                        );
                        return Ok(PublicationCoverage::Covered);
                    }
                } else {
                    tracing::debug!(
                        event = "publication.support_missing",
                        reason = "current_projection_unavailable",
                        result_id = %result.id,
                        attempt_id = %attempt.id,
                    );
                    missing = true;
                }
            }
        }
    }
    let coverage = if missing {
        PublicationCoverage::NotEstablished
    } else {
        PublicationCoverage::Pending
    };
    tracing::debug!(
        event = "publication.coverage_decided",
        ?service,
        destination_id = destination.map(tracing::field::display),
        ?coverage,
        missing_support = missing,
    );
    Ok(coverage)
}

/// Selected confirmation evidence and the current comparison basis.
pub struct ConfirmationInput<'a> {
    /// Provider-specific matching dimensions.
    pub service: Service,
    /// Current contact identity under the same transformations.
    pub current: &'a ConfirmationBasis,
    /// Values covered by the selected positive or negative report/import.
    pub basis: &'a ConfirmationBasis,
    /// Selected explicit status; absence is not a negative.
    pub status: Option<ConfirmationStatus>,
    /// Actual counterpart contact identity/time when disclosed.
    pub counterpart: Option<&'a ConfirmationBasis>,
    /// Actual counterpart station report eligible for fallback, never our own returned data.
    pub station: Option<&'a StationReport>,
}

/// Check basic confirmation applicability without applying a time tolerance between our edits.
pub fn confirmation_applicability(
    input: &ConfirmationInput<'_>,
) -> anyhow::Result<ConfirmationApplicability> {
    let Some(status) = input.status else {
        return Ok(ConfirmationApplicability::NotEstablished);
    };
    let a = input.current;
    let b = input.basis;
    if a == b {
        return Ok(if status == ConfirmationStatus::Confirmed {
            ConfirmationApplicability::Confirmed
        } else {
            ConfirmationApplicability::Unconfirmed
        });
    }
    for (a, b) in [(&a.own_call, &b.own_call), (&a.worked_call, &b.worked_call)] {
        if a.is_none() || b.is_none() {
            return Ok(ConfirmationApplicability::NotEstablished);
        }
        if a != b {
            return Ok(ConfirmationApplicability::HistoricalOnly);
        }
    }
    if a.band != b.band || (input.service == Service::Lotw && a.receive_band != b.receive_band) {
        return Ok(ConfirmationApplicability::HistoricalOnly);
    }
    if a.uninterpreted_band != b.uninterpreted_band
        || (input.service == Service::Lotw
            && a.uninterpreted_receive_band != b.uninterpreted_receive_band)
    {
        return Ok(ConfirmationApplicability::NotEstablished);
    }
    if a.band.is_none() && a.frequency != b.frequency
        || (input.service == Service::Lotw
            && a.receive_band.is_none()
            && a.receive_frequency != b.receive_frequency)
    {
        return Ok(ConfirmationApplicability::NotEstablished);
    }
    let (Some(am), Some(bm)) = (&a.mode, &b.mode) else {
        return Ok(ConfirmationApplicability::NotEstablished);
    };
    let compatible = if input.service == Service::Qrz {
        am.parent.as_ref().unwrap_or(&am.concrete) == bm.parent.as_ref().unwrap_or(&bm.concrete)
    } else {
        am.concrete == bm.concrete
            || am
                .group
                .as_ref()
                .zip(bm.group.as_ref())
                .is_some_and(|(a, b)| a == b)
    };
    if !compatible {
        return Ok(ConfirmationApplicability::HistoricalOnly);
    }
    if a.start != b.start {
        let (Some(current), Some(other)) = (a.start, input.counterpart.and_then(|p| p.start))
        else {
            return Ok(ConfirmationApplicability::NotEstablished);
        };
        let diff = i128::from(current.unix_seconds()) - i128::from(other.unix_seconds());
        if diff.abs() > 1800 {
            return Ok(ConfirmationApplicability::HistoricalOnly);
        }
        let Some(other) = input.counterpart else {
            return Ok(ConfirmationApplicability::NotEstablished);
        };
        if other.own_call.as_ref() != a.worked_call.as_ref()
            || other.worked_call.as_ref() != a.own_call.as_ref()
            || other.band != a.band
        {
            return Ok(ConfirmationApplicability::NotEstablished);
        }
    }
    Ok(if status == ConfirmationStatus::Confirmed {
        ConfirmationApplicability::Confirmed
    } else {
        ConfirmationApplicability::Unconfirmed
    })
}

/// Identify exact-mode coverage separately from basic group-level confirmation.
pub fn exact_mode_qualification(input: &ConfirmationInput<'_>) -> bool {
    input.status == Some(ConfirmationStatus::Confirmed)
        && input
            .current
            .mode
            .as_ref()
            .zip(input.basis.mode.as_ref())
            .is_some_and(|(a, b)| a.concrete == b.concrete)
        && matches!(
            confirmation_applicability(input),
            Ok(ConfirmationApplicability::Confirmed)
        )
}

/// Convert only eligible actual counterpart facts to a fallback station projection.
pub fn eligible_station_facts(
    input: &ConfirmationInput<'_>,
) -> anyhow::Result<Option<StationFacts>> {
    if confirmation_applicability(input)? != ConfirmationApplicability::Confirmed {
        return Ok(None);
    }
    let Some(s) = input.station else {
        return Ok(None);
    };
    let location = crate::location::RecordedLocationFields {
        position: reported(&s.position),
        dxcc: reported(&s.dxcc),
        primary_subdivision: reported(&s.primary_subdivision),
        secondary_subdivision: reported(&s.secondary_subdivision),
        cq_zone: reported(&s.cq_zone),
        itu_zone: reported(&s.itu_zone),
    }
    .normalized()
    .map(RecordedLocation::try_from)
    .transpose()?;
    Ok(Some(StationFacts {
        references: reported(&s.references).unwrap_or_default(),
        name: reported(&s.name),
        location,
    }))
}

/// Extract a supplied value without treating omitted/cleared metadata as a fallback fact.
fn reported<T: Clone>(value: &Reported<T>) -> Option<T> {
    if let Reported::Value(v) = value {
        Some(v.clone())
    } else {
        None
    }
}

/// Return emitted values for comparisons without request framing or signatures.
pub fn submitted_fields(projection: &SubmissionProjection) -> AdifFields {
    match projection {
        SubmissionProjection::Qrz(v) => v.fields.clone(),
        SubmissionProjection::Lotw(v) => {
            let mut fields = v.contact_fields().clone();
            fields.extend(
                v.station_fields()
                    .iter()
                    .map(|(k, v)| (format!("STATION:{k}"), v.clone())),
            );
            fields.insert(
                "STATION:CALL".into(),
                v.signing_identity().callsign.as_str().into(),
            );
            fields.insert("STATION:DXCC".into(), v.signing_identity().dxcc.clone());
            fields
        }
    }
}
