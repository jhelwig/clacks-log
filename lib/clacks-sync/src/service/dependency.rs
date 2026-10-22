//! Required retained support and conditional admission for service evidence.

use super::{ExternalBindingGuard, ServiceRecord, ServiceView, SupportRequirement};
use crate::{
    acceptance::{AcceptanceInput, RejectionReason},
    account::EvidenceView,
    compare::{RecordRelation, compare_contexts},
    record::{Observation, RecordId, RecordKey, RecordState, RecordValue},
};
use clacks_model::{
    ids::AccountId,
    service::{
        destination::RemoteRecordId,
        evidence::{
            ServicePayload, ServiceRecordId,
            processing::{AssociationState, ProcessingDecision},
            submission::ResultScope,
        },
    },
};
use serde::{Deserialize, Serialize};

/// A service integrity/scope failure, distinct from unresolved input or stale guards.
#[derive(Debug, thiserror::Error)]
pub enum SyncServiceError {
    /// Same immutable identity claims incompatible contents.
    #[error("service integrity error for {key:?}: {detail}")]
    Integrity {
        /// Account-qualified offending record.
        key: RecordKey,
        /// Contradictory value/support explanation.
        detail: String,
    },
    /// Referenced material crosses an account or provider namespace.
    #[error("service scope error in account {account}: {detail}")]
    Scope {
        /// Owning account.
        account: AccountId,
        /// Violated identity relationship.
        detail: String,
    },
}

/// Missing support that prevents application or completed coverage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingDependency {
    /// Missing or uninterpretable identity.
    pub key: RecordKey,
    /// Required interpretation or effect.
    pub reason: String,
}

/// Whether retained evidence can be interpreted/applied without fabricating missing support.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DependencyStatus {
    /// All required facts and effects are represented.
    Complete,
    /// Material may be retained/staged, but cannot justify completed effects or positive claims.
    Incomplete(Vec<MissingDependency>),
}

/// Derive mandatory semantic references even when caller-provided support omits them.
pub(crate) fn required_keys(account: AccountId, record: &ServiceRecord) -> Vec<RecordKey> {
    let key = |id| RecordKey {
        account,
        id: RecordId::Service(id),
    };
    match &record.payload {
        ServicePayload::Imported(v) => vec![RecordKey {
            account,
            id: RecordId::Qso(v.qso),
        }],
        ServicePayload::Attempt(v) => vec![RecordKey {
            account,
            id: RecordId::Qso(v.qso),
        }],
        ServicePayload::Result(v) => v
            .scope
            .attempts()
            .into_iter()
            .map(|id| key(ServiceRecordId::Attempt(id)))
            .collect(),
        ServicePayload::Disposition(v) => {
            std::iter::once(key(ServiceRecordId::Observation(v.observation)))
                .chain(
                    v.association
                        .map(|id| key(ServiceRecordId::Association(id))),
                )
                .collect()
        }
        ServicePayload::Association(v) if v.state != AssociationState::Superseded => {
            vec![RecordKey {
                account,
                id: RecordId::Qso(v.qso),
            }]
        }
        _ => Vec::new(),
    }
}

/// Check semantic references, explicit effect coverage and public namespace interpretation.
pub fn check(record: &ServiceRecord, view: &ServiceView<'_>) -> anyhow::Result<DependencyStatus> {
    record.validate(view.account)?;
    if view.evidence.lineage.account() != view.account {
        return Err(SyncServiceError::Scope {
            account: view.account,
            detail: "lineage belongs to another account".into(),
        }
        .into());
    }
    for row in view.records {
        if row.key.account != view.account {
            return Err(SyncServiceError::Scope {
                account: view.account,
                detail: format!("cross-account dependency {:?}", row.key),
            }
            .into());
        }
        row.validate()?;
    }
    let mut missing = Vec::new();
    for key in required_keys(view.account, record) {
        let found = view.records.iter().find(|r| r.key == key);
        if found.is_none() {
            missing.push(MissingDependency {
                key,
                reason: "required semantic support missing".into(),
            });
        }
        if let Some(row) = found {
            if let ServicePayload::Association(a) = &record.payload {
                let good = match a.state {
                    AssociationState::Active => {
                        matches!(row.state, RecordState::Live(RecordValue::Qso(_)))
                    }
                    AssociationState::DeletedQso => matches!(row.state, RecordState::Deleted),
                    AssociationState::Superseded => true,
                };
                if !good {
                    missing.push(MissingDependency {
                        key,
                        reason: "association target state differs".into(),
                    });
                }
            }
            if let ServicePayload::Result(r) = &record.payload
                && let Some(s) = super::payload(row)
                && let ServicePayload::Attempt(a) = &s.payload
                && (a.projection.destination() != r.destination
                    || matches!(&r.scope,ResultScope::UploadBatch {batch,..} if a.batch!=Some(*batch)))
            {
                return Err(SyncServiceError::Integrity {
                    key,
                    detail: format!("result {} has mismatched attempt/batch destination", r.id),
                }
                .into());
            }
        }
    }
    for requirement in &record.support.required {
        let required = requirement.reference();
        let represented = view
            .records
            .iter()
            .filter(|v| v.key == required.key)
            .any(|actual| {
                let relation =
                    compare_contexts(required.key, &actual.stamp, &required.stamp, view.evidence);
                match requirement {
                    SupportRequirement::ExactFact(_) => {
                        matches!(relation, Ok(RecordRelation::SameVersion))
                            && matches!(actual.state, RecordState::Deleted) == required.deleted
                    }
                    SupportRequirement::CoveredEffect(_) => matches!(
                        relation,
                        Ok(RecordRelation::After | RecordRelation::SameVersion)
                    ),
                }
            });
        if !represented {
            missing.push(MissingDependency {
                key: required.key,
                reason: "accepted support/effect version is not represented".into(),
            });
        }
    }
    if let Some(id) = record.payload.destination()
        && !matches!(&record.payload, ServicePayload::Destination(_))
    {
        let described = record.support.destination.as_ref().or_else(|| {
            view.records
                .iter()
                .filter_map(super::payload)
                .find_map(|r| match &r.payload {
                    ServicePayload::Destination(d) if d.id == id => Some(d),
                    _ => None,
                })
        });
        if let Some(d) = described {
            let consistent = match &record.payload {
                ServicePayload::Imported(v) => d.service() == v.service,
                ServicePayload::Attempt(v) => d.service() == v.projection.service(),
                ServicePayload::Observation(v) => match &v.identity {
                    clacks_model::service::evidence::observation::RemoteIdentity::Complete(k) => {
                        matches!(
                            (&k.record, d.service()),
                            (
                                RemoteRecordId::Lotw(_),
                                clacks_model::service::destination::Service::Lotw
                            ) | (
                                RemoteRecordId::Qrz(_),
                                clacks_model::service::destination::Service::Qrz
                            )
                        )
                    }
                    clacks_model::service::evidence::observation::RemoteIdentity::Incomplete {
                        service,
                        ..
                    } => *service == d.service(),
                },
                _ => true,
            };
            if !consistent {
                return Err(SyncServiceError::Scope {
                    account: view.account,
                    detail: format!("destination {id} does not match evidence provider"),
                }
                .into());
            }
        } else {
            missing.push(MissingDependency {
                key: RecordKey {
                    account: view.account,
                    id: RecordId::Service(ServiceRecordId::Destination(id)),
                },
                reason: "public destination description missing".into(),
            });
        }
    }
    if let ServicePayload::Disposition(d) = &record.payload
        && matches!(
            d.decision,
            ProcessingDecision::Added | ProcessingDecision::Associated
        )
    {
        let associated = d.association.and_then(|id| {
            view.records
                .iter()
                .filter_map(super::payload)
                .find_map(|r| match &r.payload {
                    ServicePayload::Association(a) if a.id == id => Some(a),
                    _ => None,
                })
        });
        if let Some(a) = associated {
            let qso = RecordKey {
                account: view.account,
                id: RecordId::Qso(a.qso),
            };
            if !record
                .support
                .required
                .iter()
                .any(|r| matches!(r,SupportRequirement::CoveredEffect(v) if v.key==qso))
            {
                missing.push(MissingDependency {
                    key: qso,
                    reason: "completed disposition lacks an accepted QSO effect reference".into(),
                });
            }
        } else {
            missing.push(MissingDependency {
                key: RecordKey {
                    account: view.account,
                    id: RecordId::Service(ServiceRecordId::Disposition(d.id)),
                },
                reason: "completed association decision has no represented binding".into(),
            });
        }
    }
    Ok(if missing.is_empty() {
        DependencyStatus::Complete
    } else {
        DependencyStatus::Incomplete(missing)
    })
}

/// Enforce evidence requirements and unique-index conditions inside generic acceptance.
pub(crate) fn admission(
    state: &RecordState,
    input: &AcceptanceInput<'_>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Result<Vec<ExternalBindingGuard>, RejectionReason>> {
    let RecordState::Live(RecordValue::Service(record)) = state else {
        return Ok(Ok(Vec::new()));
    };
    let rows = input
        .references
        .iter()
        .chain(std::iter::once(input.current))
        .filter_map(|o| {
            if let Observation::Accepted(v) = o {
                Some(v.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let view = ServiceView {
        account: input.account.account,
        records: &rows,
        bindings: input.bindings,
        evidence,
    };
    if let DependencyStatus::Incomplete(missing) = check(record, &view)? {
        return Ok(Err(RejectionReason::ServiceCondition {
            detail: format!("incomplete service support: {missing:?}"),
        }));
    }
    let external = match &record.payload {
        ServicePayload::Association(v) => Some((&v.external, Some(v))),
        ServicePayload::Suppression(v) => Some((&v.external, None)),
        _ => None,
    };
    let Some((external, association)) = external else {
        return Ok(Ok(Vec::new()));
    };
    let matches = input
        .bindings
        .iter()
        .filter(|b| b.account == input.account.account && &b.external == external)
        .collect::<Vec<_>>();
    let Some(read) = matches.first().filter(|_| matches.len() == 1) else {
        return Ok(Err(RejectionReason::ServiceCondition {
            detail: format!(
                "external binding {external:?} requires one explicit index observation"
            ),
        }));
    };
    if let Some(a) = association
        && read.binding.as_ref().is_some_and(|b| {
            b.association != a.id
                || b.qso != a.qso
                || (b.state != AssociationState::Active && a.state == AssociationState::Active)
        })
    {
        return Ok(Err(RejectionReason::ServiceCondition {
            detail: format!(
                "external record {external:?} is occupied; explicit resolution required"
            ),
        }));
    }
    Ok(Ok(vec![ExternalBindingGuard {
        account: read.account,
        external: external.clone(),
        expected: read.binding.clone(),
    }]))
}
