//! Original accepted-evidence installation through an authorized hub-and-spoke carrier.

use super::{
    ExternalBindingObservation,
    dependency::required_keys,
    operation::{CoupledCommit, CoupledRequirement, ServiceDecision},
};
use crate::{
    acceptance::AcceptanceInput,
    account::{AccountMetadata, EvidenceView},
    client::{OriginalBasis, VersionCondition},
    membership::MembershipSnapshot,
    record::{AcceptedRecord, Observation},
    recovery::{RecoveredDecision, plan_recovered_install},
};
use clacks_model::ids::ClientActorId;
use std::collections::BTreeSet;

/// Accepted unit and its complete interpretation/effect support from one authenticated client.
pub struct RecoveredOperationInput<'a> {
    /// Complete destination registry observed at the restored account head.
    pub destination_registry: &'a super::DestinationRegistryObservation,
    /// Original unit descriptor, never regenerated from local wall time.
    pub unit: &'a CoupledRequirement,
    /// Original accepted envelopes, including any accompanying supporting facts.
    pub incoming: &'a [AcceptedRecord],
    /// Currently authorized client returning server-distributed state.
    pub returning_actor: ClientActorId,
    /// Current restored account metadata.
    pub account: &'a AccountMetadata,
    /// Current admission registry, independent of the original authors.
    pub membership: &'a MembershipSnapshot,
    /// Current transactional target/support observations, including certified absences.
    pub reads: &'a [Observation],
    /// Current unique external-binding index observations.
    pub bindings: &'a [ExternalBindingObservation],
    /// Available original comparison values for maintained-state reconciliation.
    pub originals: &'a [OriginalBasis],
    /// Available timeline/baseline interpretation.
    pub evidence: &'a EvidenceView<'a>,
}

/// Plan all recovered effects atomically while preserving each original acceptance stamp.
///
/// Incomplete/review outcomes require retaining the supplied input for retry. This
/// operation does not dispatch a request or certify an old complete prefix.
pub fn plan_recovered_operation(
    input: &RecoveredOperationInput<'_>,
) -> anyhow::Result<ServiceDecision<CoupledCommit>> {
    input.membership.validate(input.account)?;
    if crate::acceptance::admission(input.returning_actor, input.membership).is_some() {
        return Ok(ServiceDecision::Review(
            "returning client is not currently authorized".into(),
        ));
    }
    anyhow::ensure!(
        !input.unit.members.is_empty(),
        "recovered operation has no members"
    );
    let mut read_keys = BTreeSet::new();
    for row in input.reads {
        row.validate()?;
        anyhow::ensure!(
            row.key().account == input.account.account && read_keys.insert(row.key()),
            "duplicate or cross-account recovery read"
        );
    }
    let mut incoming_keys = BTreeSet::new();
    for row in input.incoming {
        row.validate()?;
        anyhow::ensure!(
            row.key.account == input.account.account && incoming_keys.insert(row.key),
            "duplicate or cross-account recovered record"
        );
    }
    anyhow::ensure!(
        input
            .unit
            .members
            .iter()
            .all(|r| r.key.account == input.account.account),
        "unit crosses account"
    );
    anyhow::ensure!(
        input.destination_registry.account == input.account.account
            && input.destination_registry.head == input.account.head,
        "recovery destination registry account/head differs"
    );
    let mut registry = input.destination_registry.clone();
    let mut pending = input.incoming.to_vec();
    let mut prospective = input.reads.to_vec();
    let mut next_account = input.account.clone();
    let mut accepted = Vec::new();
    let mut journals = Vec::new();
    let mut conditions = Vec::new();
    let mut external_guards = Vec::new();
    while !pending.is_empty() {
        let candidates: Vec<_> = pending
            .iter()
            .map(|row| {
                let mut keys = crate::acceptance::required_references(row.key, &row.state);
                if let Some(s) = super::payload(row) {
                    keys.extend(required_keys(input.account.account, s));
                    keys.extend(s.support.required.iter().map(|r| r.reference().key));
                }
                (row.key, &row.state, keys)
            })
            .collect();
        let priority = super::namespace_prerequisites(&registry.destinations, &candidates);
        let ready = candidates
            .iter()
            .enumerate()
            .filter(|(_, (_, _, required))| {
                required
                    .iter()
                    .all(|key| !candidates.iter().any(|(candidate, _, _)| candidate == key))
            })
            .min_by_key(|(_, (key, _, _))| !priority.contains(key))
            .map(|(index, _)| index);
        let Some(index) = ready else {
            return Ok(ServiceDecision::Incomplete(
                "recovered dependency cycle requires support/covering effects".into(),
            ));
        };
        let row = pending.remove(index);
        let Some(current) = prospective.iter().find(|r| r.key() == row.key).cloned() else {
            return Ok(ServiceDecision::Incomplete(format!(
                "explicit target read missing: {:?}",
                row.key
            )));
        };
        if let Some(original) = input.reads.iter().find(|r| r.key() == row.key) {
            conditions.push(VersionCondition::from_observation(original)?);
        }
        registry.head = next_account.head;
        let source = AcceptanceInput {
            destination_registry: Some(&registry),
            account: &next_account,
            membership: input.membership,
            current: &current,
            references: &prospective,
            bindings: input.bindings,
        };
        match plan_recovered_install(
            &row,
            input.originals.iter().find(|o| o.key() == row.key),
            input.returning_actor,
            &source,
            input.evidence,
        )? {
            RecoveredDecision::Install {
                external_guards: guards,
                dependencies,
                envelope,
                journal,
                next_account: next,
                ..
            } => {
                external_guards.extend(guards);
                conditions.extend(
                    dependencies
                        .into_iter()
                        .filter(|d| !incoming_keys.contains(&d.key())),
                );
                next_account = next;
                prospective.retain(|r| r.key() != envelope.key);
                prospective.push(Observation::Accepted(envelope.clone()));
                super::retain_destination(&mut registry.destinations, &envelope);
                accepted.push(envelope);
                journals.push(*journal);
            }
            RecoveredDecision::Reconciled(c) => {
                external_guards.extend(c.external_guards);
                conditions.extend(
                    c.dependencies
                        .into_iter()
                        .filter(|d| !incoming_keys.contains(&d.key())),
                );
                next_account = c.next_account;
                prospective.retain(|r| r.key() != c.accepted.key);
                prospective.push(Observation::Accepted(c.accepted.clone()));
                super::retain_destination(&mut registry.destinations, &c.accepted);
                accepted.push(c.accepted);
                journals.push(c.journal);
            }
            RecoveredDecision::Covered => {}
            RecoveredDecision::Review(r) => {
                return Ok(ServiceDecision::Review(format!(
                    "maintained/native recovery requires client resolution: {:?}",
                    r.reason
                )));
            }
            RecoveredDecision::DeferredService { detail } => {
                return Ok(ServiceDecision::Incomplete(detail));
            }
        }
    }
    for member in &input.unit.members {
        let Some(Observation::Accepted(actual)) =
            prospective.iter().find(|r| r.key() == member.key)
        else {
            return Ok(ServiceDecision::Incomplete(format!(
                "unit member missing {:?}",
                member.key
            )));
        };
        let relation = crate::compare::compare_contexts(
            member.key,
            &actual.stamp,
            &member.stamp,
            input.evidence,
        )?;
        if !matches!(
            relation,
            crate::compare::RecordRelation::SameVersion | crate::compare::RecordRelation::After
        ) {
            return Ok(ServiceDecision::Incomplete(format!(
                "unit member not covered {:?}",
                member.key
            )));
        }
        if matches!(relation, crate::compare::RecordRelation::SameVersion) {
            anyhow::ensure!(
                member.deleted == matches!(actual.state, crate::record::RecordState::Deleted),
                "recovered atomic deletion state differs"
            );
        }
        if !incoming_keys.contains(&member.key)
            && let Some(original) = input.reads.iter().find(|r| r.key() == member.key)
        {
            conditions.push(VersionCondition::from_observation(original)?);
        }
    }
    if accepted.is_empty() {
        return Ok(ServiceDecision::Unchanged);
    }
    // The original unit describes covered effects, not newly allocated installation positions.
    let unit = CoupledRequirement {
        id: input.unit.id,
        members: input.unit.members.clone(),
    };
    Ok(ServiceDecision::Ready(CoupledCommit {
        expected_head: input.account.head,
        expected_membership_generation: input.membership.generation,
        conditions,
        external_guards,
        next_account,
        accepted,
        journals,
        unit,
        job_updates: vec![],
    }))
}
