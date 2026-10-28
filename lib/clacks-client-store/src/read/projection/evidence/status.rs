//! Publication coverage in retained service-specific destination contexts.

use super::{Evaluated, Evidence, ReadContext};
use clacks_model::{
    contact::Qso,
    effective::EligibleStationFacts,
    ids::DestinationId,
    service::{
        adif::ProjectionContext,
        applicability::{PublicationCoverage, PublicationCoverageInput, publication_coverage},
        destination::{Destination, RemoteRecordId, Service},
        error::ProjectionDecision,
        evidence::{
            ImportedBaselineInput, ServicePayload, imported_baseline, processing::AssociationState,
            submission::SubmissionProjection,
        },
        lotw::{self, LotwProjectionInput},
        qrz::{self, QrzProjectionInput},
    },
};
use std::collections::BTreeSet;

/// Find a retained public descriptor without depending on a current credential.
fn destination(evidence: &Evidence, id: DestinationId) -> Option<&Destination> {
    evidence
        .working
        .iter()
        .find_map(|r| r.support.destination.as_ref().filter(|d| d.id == id))
        .or_else(|| {
            evidence.working.iter().find_map(|r| {
                if let ServicePayload::Destination(d) = &r.payload {
                    (d.id == id).then_some(d)
                } else {
                    None
                }
            })
        })
}

/// Resolve one QRZ book using actual association first, then unique retained publication context.
pub(super) fn qrz_destination(
    qso: &Qso,
    evidence: &Evidence,
) -> Result<Option<DestinationId>, &'static str> {
    let associations: BTreeSet<_> = evidence
        .working
        .iter()
        .filter_map(|r| {
            if let ServicePayload::Association(a) = &r.payload {
                (a.qso == qso.id
                    && a.state == AssociationState::Active
                    && matches!(a.external.record, RemoteRecordId::Qrz(_)))
                .then_some(a.external.destination)
            } else {
                None
            }
        })
        .collect();
    if associations.len() > 1 {
        return Err("Multiple active QRZ books need reconciliation");
    }
    if let Some(id) = associations.first() {
        return Ok(Some(*id));
    }
    let candidates: BTreeSet<_> = evidence
        .working
        .iter()
        .filter_map(|r| match &r.payload {
            ServicePayload::Attempt(a)
                if a.qso == qso.id && a.projection.service() == Service::Qrz =>
            {
                Some(a.projection.destination())
            }
            ServicePayload::Imported(a) if a.qso == qso.id && a.service == Service::Qrz => {
                a.destination
            }
            _ => None,
        })
        .collect();
    if candidates.len() > 1 {
        return Err("QRZ publication evidence has ambiguous book context");
    }
    Ok(candidates.first().copied())
}

/// Compute publication against actual frozen submission semantics and imported source baselines.
pub(super) fn evaluate(
    qso: &Qso,
    evidence: &Evidence,
    context: &ReadContext,
    projection: &ProjectionContext<'_>,
    result: &mut Evaluated,
) -> anyhow::Result<()> {
    let all_assertions: Vec<_> = evidence
        .working
        .iter()
        .filter_map(|r| {
            if let ServicePayload::Imported(a) = &r.payload {
                Some((**a).clone())
            } else {
                None
            }
        })
        .collect();
    let attempts: Vec<_> = evidence
        .working
        .iter()
        .filter_map(|r| {
            if let ServicePayload::Attempt(a) = &r.payload {
                Some(a.clone())
            } else {
                None
            }
        })
        .collect();
    let results: Vec<_> = evidence
        .working
        .iter()
        .filter_map(|r| {
            if let ServicePayload::Result(r) = &r.payload {
                Some(r.clone())
            } else {
                None
            }
        })
        .collect();
    for service in [Service::Lotw, Service::Qrz] {
        let baseline = imported_baseline(&ImportedBaselineInput {
            qso,
            service,
            coverage: Default::default(),
            lotw: &context.lotw,
            context: projection,
        })?;
        let selected = if service == Service::Qrz {
            qrz_destination(qso, evidence)
        } else {
            Ok(None)
        };
        let ambiguous = selected.is_err();
        let selected = match selected {
            Ok(id) => id,
            Err(reason) => {
                result.facts.diagnostics.push(reason.into());
                None
            }
        };
        let assertions: Vec<_> = all_assertions
            .iter()
            .filter(|a| !ambiguous || a.destination.is_none())
            .cloned()
            .collect();
        let imported = publication_coverage(&PublicationCoverageInput {
            qso: qso.id,
            service,
            destination: selected,
            current: None,
            source_current: &baseline,
            assertions: &assertions,
            attempts: &[],
            results: &[],
        })? == PublicationCoverage::Covered;
        let mut covered = imported;
        if !ambiguous {
            for attempt in attempts.iter().filter(|a| {
                a.qso == qso.id
                    && a.projection.service() == service
                    && selected.is_none_or(|id| a.projection.destination() == id)
            }) {
                let id = attempt.projection.destination();
                let Some(destination) = destination(evidence, id) else {
                    result.facts.diagnostics.push(format!(
                        "{service:?} submission lacks its public destination descriptor"
                    ));
                    continue;
                };
                let current = match &attempt.projection {
                    SubmissionProjection::Lotw(original) => {
                        match lotw::project(&LotwProjectionInput {
                            qso,
                            destination,
                            signing: &original.signing_identity,
                            configuration: &context.lotw,
                            definitions: &context.definitions,
                        })? {
                            ProjectionDecision::Ready(v) => {
                                Some(SubmissionProjection::Lotw(Box::new(v.value)))
                            }
                            ProjectionDecision::NotReady(_) => None,
                        }
                    }
                    SubmissionProjection::Qrz(_) => match qrz::project(&QrzProjectionInput {
                        qso,
                        destination,
                        eligible: EligibleStationFacts {
                            lotw: result.lotw.as_ref(),
                            qrz: result.qrz.as_ref(),
                            directory: None,
                        },
                        context: projection,
                    })? {
                        ProjectionDecision::Ready(v) => Some(SubmissionProjection::Qrz(v.value)),
                        ProjectionDecision::NotReady(_) => None,
                    },
                };
                if current.is_none() {
                    result.facts.diagnostics.push(format!(
                        "{service:?} current submission values could not be established"
                    ));
                }
                covered |= publication_coverage(&PublicationCoverageInput {
                    qso: qso.id,
                    service,
                    destination: Some(id),
                    current: current.as_ref(),
                    source_current: &baseline,
                    assertions: &assertions,
                    attempts: &attempts,
                    results: &results,
                })? == PublicationCoverage::Covered;
            }
        }
        match service {
            Service::Lotw => result.facts.lotw.uploaded = covered,
            Service::Qrz => result.facts.qrz.uploaded = covered,
        };
    }
    Ok(())
}
