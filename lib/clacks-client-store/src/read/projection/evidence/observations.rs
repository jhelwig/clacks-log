//! Confirmation applicability and eligible counterpart facts from retained evidence.

use super::{Evaluated, Evidence, ReadContext};
use crate::read::export::ConfirmationDate;
use clacks_model::{
    contact::Qso,
    service::{
        adif::ProjectionContext,
        applicability::{
            ConfirmationApplicability, ConfirmationInput, confirmation_applicability,
            eligible_station_facts,
        },
        destination::{RemoteRecordId, Service},
        evidence::{
            ImportedBaselineInput, ImportedClaim, ServicePayload, imported_baseline,
            observation::{ConfirmationStatus, ContactReport, ObservedStart, Reported},
            processing::{AssociationState, ProcessingDecision},
            submission::ConfirmationBasis,
        },
    },
};
use clacks_sync::{
    record::{RecordState, RecordValue},
    service::{
        ServiceView,
        combination::{self, ObservationSelectionInput},
    },
};

/// Borrow a supplied report value without turning omission or clear into a fact.
fn reported<T>(value: &Reported<T>) -> Option<&T> {
    if let Reported::Value(value) = value {
        Some(value)
    } else {
        None
    }
}

/// Capture only the contact dimensions the provider actually supplied.
fn report_basis(report: &ContactReport) -> ConfirmationBasis {
    ConfirmationBasis {
        own_call: reported(&report.own_call).cloned(),
        worked_call: reported(&report.worked_call).cloned(),
        start: match reported(&report.start) {
            Some(ObservedStart::Second(t)) => Some(*t),
            _ => None,
        },
        band: reported(&report.band).cloned(),
        receive_band: reported(&report.receive_band).cloned(),
        frequency: reported(&report.frequency).copied(),
        receive_frequency: reported(&report.receive_frequency).copied(),
        mode: reported(&report.mode).cloned(),
        ..Default::default()
    }
}

/// Derive imported and associated confirmation without treating echo data as counterpart evidence.
pub(super) fn evaluate(
    qso: &Qso,
    evidence: &Evidence,
    context: &ReadContext,
    projection: &ProjectionContext<'_>,
    result: &mut Evaluated,
) -> anyhow::Result<()> {
    for service in [Service::Lotw, Service::Qrz] {
        let mut positive_imports = Vec::new();
        let book = if service == Service::Qrz {
            super::status::qrz_destination(qso, evidence)
        } else {
            Ok(None)
        };
        let current = imported_baseline(&ImportedBaselineInput {
            qso,
            service,
            coverage: Default::default(),
            lotw: &context.lotw,
            context: projection,
        })?;
        for row in &evidence.working {
            let ServicePayload::Imported(assertion) = &row.payload else {
                continue;
            };
            if assertion.qso != qso.id()
                || assertion.service != service
                || assertion.claim != ImportedClaim::Confirmed
                || (service == Service::Qrz
                    && assertion.destination.is_some()
                    && match book {
                        Ok(Some(id)) => assertion.destination != Some(id),
                        Ok(None) => false,
                        Err(_) => true,
                    })
            {
                continue;
            }
            if confirmation_applicability(&ConfirmationInput {
                service,
                current: &current.confirmation,
                basis: &assertion.baseline.confirmation,
                status: Some(ConfirmationStatus::Confirmed),
                counterpart: None,
                station: None,
            })? == ConfirmationApplicability::Confirmed
            {
                positive_imports.push(assertion.as_ref());
                if let Some(date) = assertion.date {
                    result.confirmation_dates.push(ConfirmationDate {
                        service,
                        date,
                        provenance: assertion.provenance.clone(),
                    });
                }
                match service {
                    Service::Lotw => result.facts.lotw.confirmed = true,
                    Service::Qrz => result.facts.qrz.confirmed = true,
                };
            }
        }
        let associations: Vec<_> = evidence
            .working
            .iter()
            .filter_map(|r| {
                if let ServicePayload::Association(a) = &r.payload {
                    Some(a)
                } else {
                    None
                }
            })
            .filter(|a| {
                a.qso == qso.id()
                    && a.state == AssociationState::Active
                    && matches!(
                        (&a.external.record, service),
                        (RemoteRecordId::Lotw(_), Service::Lotw)
                            | (RemoteRecordId::Qrz(_), Service::Qrz)
                    )
            })
            .collect();
        if associations.len() > 1 {
            result.facts.diagnostics.push(format!(
                "Multiple active {service:?} record associations need reconciliation"
            ));
            continue;
        }
        let Some(association) = associations.first() else {
            continue;
        };
        let Some(interpretation) = &evidence.interpretation else {
            result.facts.diagnostics.push(format!(
                "{service:?} observation ordering has no installed synchronization interpretation"
            ));
            continue;
        };
        let interpretation = interpretation.view();
        let view = ServiceView {
            account: interpretation.lineage.account(),
            records: &evidence.accepted,
            bindings: &[],
            evidence: &interpretation,
        };
        let selected = combination::select_observations(&ObservationSelectionInput {
            external: &association.external,
            view: &view,
        })?;
        if !selected.uncertain.is_empty() {
            result.facts.diagnostics.push(format!(
                "{service:?} retained observations have {} unresolved fields",
                selected.uncertain.len()
            ));
        }
        let mut basis = report_basis(&selected.current.facts.contact);
        // A processing disposition can retain the precise local confirmed baseline,
        // even when the provider reported only a minute-wide timestamp.
        let baselines: Vec<_> = evidence
            .working
            .iter()
            .filter(|r| {
                let ServicePayload::Disposition(d) = &r.payload else {
                    return false;
                };
                d.association == Some(association.id)
                    && selected.confirmation_supporting.contains(&d.observation)
                    && matches!(
                        d.decision,
                        ProcessingDecision::Associated
                            | ProcessingDecision::Added
                            | ProcessingDecision::CompletedEquivalent
                    )
            })
            .flat_map(|r| &r.support.required)
            .map(|r| r.reference())
            .filter(|r| r.key.id == clacks_sync::record::RecordId::Qso(qso.id()))
            .collect();
        let original: Vec<_> = evidence
            .accepted
            .iter()
            .filter(|r| {
                baselines
                    .iter()
                    .any(|b| b.key == r.key() && b.stamp == r.stamp)
            })
            .filter_map(|r| {
                if let RecordState::Live(RecordValue::Qso(q)) = &r.state() {
                    Some(q.as_ref())
                } else {
                    None
                }
            })
            .collect();
        if let Some(first) = original.first() {
            let retained = imported_baseline(&ImportedBaselineInput {
                qso: first,
                service,
                coverage: Default::default(),
                lotw: &context.lotw,
                context: projection,
            })?;
            let mut consistent = true;
            for other in original.iter().skip(1) {
                let other = imported_baseline(&ImportedBaselineInput {
                    qso: other,
                    service,
                    coverage: Default::default(),
                    lotw: &context.lotw,
                    context: projection,
                })?;
                consistent &= other.confirmation == retained.confirmation;
            }
            if consistent {
                basis = retained.confirmation;
            } else {
                result.facts.diagnostics.push(format!(
                    "{service:?} confirmation has incompatible retained QSO baselines"
                ));
                continue;
            }
        }
        let input = ConfirmationInput {
            service,
            current: &current.confirmation,
            basis: &basis,
            status: reported(&selected.current.facts.contact.confirmation).copied(),
            counterpart: None,
            station: selected.current.facts.counterpart.as_ref(),
        };
        let applicability = confirmation_applicability(&input)?;
        if applicability == ConfirmationApplicability::Unconfirmed && !positive_imports.is_empty() {
            if acquired_after_imports(
                evidence,
                &selected.confirmation_supporting,
                &positive_imports,
            ) {
                result
                    .confirmation_dates
                    .retain(|claim| claim.service != service);
                match service {
                    Service::Lotw => result.facts.lotw.confirmed = false,
                    Service::Qrz => result.facts.qrz.confirmed = false,
                };
            } else {
                result.facts.diagnostics.push(format!(
                    "{service:?} negative report cannot be ordered after imported confirmation"
                ));
            }
        }
        if applicability == ConfirmationApplicability::Confirmed {
            let facts = eligible_station_facts(&input)?;
            match service {
                Service::Lotw => {
                    result.facts.lotw.confirmed = true;
                    result.lotw = facts;
                }
                Service::Qrz => {
                    result.facts.qrz.confirmed = true;
                    result.qrz = facts;
                }
            }
        }
    }
    Ok(())
}

/// Establish a fresh negative read after every otherwise applicable imported positive.
///
/// Pending imports or missing/divergent lineage cannot be superseded by response
/// arrival time. Use the retained acquisition context and existing timeline rules.
fn acquired_after_imports(
    evidence: &Evidence,
    contributors: &[clacks_model::ids::ObservationId],
    imports: &[&clacks_model::service::evidence::ImportedAssertion],
) -> bool {
    use clacks_sync::lineage::PositionRelation;
    let Some(interpretation) = &evidence.interpretation else {
        return false;
    };
    imports.iter().all(|assertion| {
        evidence.accepted.iter().any(|accepted| {
            let RecordState::Live(RecordValue::Service(value)) = &accepted.state() else {
                return false;
            };
            let ServicePayload::Imported(value) = &value.payload else {
                return false;
            };
            if value.as_ref() != *assertion {
                return false;
            }
            evidence
                .working
                .iter()
                .filter(|record| match &record.payload {
                    ServicePayload::Observation(o) => contributors.contains(&o.id),
                    ServicePayload::Disposition(d) => contributors.contains(&d.observation),
                    _ => false,
                })
                .filter_map(|r| r.support.acquisition.as_ref())
                .any(|read| {
                    matches!(
                        interpretation
                            .lineage
                            .compare(accepted.stamp.accepted_at(), read.accepted_at()),
                        PositionRelation::Before | PositionRelation::Equal
                    )
                })
        })
    })
}
