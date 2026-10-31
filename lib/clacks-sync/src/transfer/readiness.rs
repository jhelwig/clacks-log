//! Dependency readiness shared by client installation and server recovery.

use super::{RecordRequirement, satisfies};
use crate::{
    account::EvidenceView,
    compare::{RecordRelation, compare_contexts},
    record::AcceptedRecord,
    service::operation::CoupledRequirement,
};
use clacks_model::ids::{AccountId, OperationId};
use std::collections::BTreeSet;

/// Accepted candidates and actual prerequisites from one receiver transaction.
pub struct ReadinessInput<'a> {
    /// Owning account.
    pub account: AccountId,
    /// Incoming or previously staged values to classify.
    pub candidates: &'a [AcceptedRecord],
    /// Current accepted observations used for semantic prerequisite checks.
    pub heads: &'a [AcceptedRecord],
    /// Exact retained prerequisites, without promoting them to current heads.
    pub support: &'a [AcceptedRecord],
    /// Related effects that must be represented together.
    pub units: &'a [CoupledRequirement],
}

/// Values eligible for installation and values still waiting for prerequisites.
pub struct ReadinessPlan {
    /// Candidates whose dependencies are represented.
    pub ready: Vec<AcceptedRecord>,
    /// Candidates which must remain durable without applying their effects.
    pub staged: Vec<AcceptedRecord>,
    /// Coupled units whose member requirements are represented.
    pub completed_units: BTreeSet<OperationId>,
}

/// Classify accepted values without constructing client edits or applying server recovery.
pub fn plan_readiness(
    input: &ReadinessInput<'_>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<ReadinessPlan> {
    for row in input
        .candidates
        .iter()
        .chain(input.heads)
        .chain(input.support)
    {
        row.validate()?;
        anyhow::ensure!(
            row.key().account == input.account,
            "readiness crosses account"
        );
    }
    let mut available = input.candidates.to_vec();
    for value in input.heads.iter().chain(input.support) {
        if !available.contains(value) {
            available.push(value.clone());
        }
    }
    // Semantic prerequisites use the newest interpreted head; exact facts remain available too.
    for index in 0..available.len() {
        let Some(value) = available.get(index).cloned() else {
            continue;
        };
        if let Some(prior) = available.iter().take(index).position(|old| {
            old.key == value.key
                && matches!(
                    compare_contexts(value.key, &value.stamp, &old.stamp, evidence),
                    Ok(RecordRelation::After)
                )
        }) {
            available.remove(index);
            available.insert(prior, value);
        }
    }
    let view = crate::service::ServiceView {
        account: input.account,
        records: &available,
        bindings: &[],
        evidence,
    };
    let mut blocked = BTreeSet::new();
    for row in input.candidates {
        if evidence
            .baseline(input.account, row.stamp.baseline())?
            .is_none()
        {
            blocked.insert(row.key);
        }
        let represented = available.iter().find(|r| r.key == row.key).unwrap_or(row);
        if let Some(s) = crate::service::payload(represented)
            && matches!(
                crate::service::dependency::check(s, &view)?,
                crate::service::dependency::DependencyStatus::Incomplete(_)
            )
        {
            blocked.insert(row.key);
        }
    }
    loop {
        let before = blocked.len();
        for unit in input.units {
            let mut complete = true;
            for member in &unit.members {
                let required = RecordRequirement {
                    key: member.key,
                    stamp: member.stamp.clone(),
                    deleted: member.deleted,
                };
                let Some(actual) = available.iter().find(|r| r.key == member.key) else {
                    complete = false;
                    continue;
                };
                if blocked.contains(&member.key) || !satisfies(&required, actual, evidence)? {
                    complete = false;
                }
            }
            if !complete {
                for member in &unit.members {
                    let relation = available
                        .iter()
                        .find(|r| r.key == member.key)
                        .map(|r| compare_contexts(member.key, &member.stamp, &r.stamp, evidence))
                        .transpose()?;
                    let durable = input
                        .heads
                        .iter()
                        .find(|r| r.key == member.key)
                        .map(|r| {
                            satisfies(
                                &RecordRequirement {
                                    key: member.key,
                                    stamp: member.stamp.clone(),
                                    deleted: member.deleted,
                                },
                                r,
                                evidence,
                            )
                        })
                        .transpose()?
                        .unwrap_or(false);
                    if !durable && !matches!(relation, Some(RecordRelation::Before)) {
                        blocked.insert(member.key);
                    }
                }
            }
        }
        for row in input.candidates {
            if let Some(s) = crate::service::payload(row) {
                let deps = crate::service::dependency::required_keys(input.account, s)
                    .into_iter()
                    .chain(s.support.required.iter().map(|r| r.reference().key));
                if deps.into_iter().any(|key| blocked.contains(&key)) {
                    blocked.insert(row.key);
                }
            }
        }
        if blocked.len() == before {
            break;
        }
    }
    let staged = input
        .candidates
        .iter()
        .filter(|r| blocked.contains(&r.key))
        .cloned()
        .collect();
    let mut completed_units = BTreeSet::new();
    for unit in input.units {
        if unit.members.iter().all(|r| !blocked.contains(&r.key)) {
            completed_units.insert(unit.id);
        }
    }
    let ready = input
        .candidates
        .iter()
        .filter(|r| !blocked.contains(&r.key))
        .cloned()
        .collect::<Vec<_>>();
    Ok(ReadinessPlan {
        ready,
        staged,
        completed_units,
    })
}
