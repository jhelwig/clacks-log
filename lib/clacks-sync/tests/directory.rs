//! Directory value policy must preserve independent key winners across ordinary receipts.

use crate::support::{self, id};
use clacks_model::{
    directory::{
        record::DirectoryRecord,
        values::{
            DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields,
            LookupKey,
        },
    },
    scalar::{RecordedCallsign, UtcTimestamp},
};
use clacks_sync::{
    account::{AccountMetadata, EvidenceView},
    client::{
        ClientRecord, ClientRecordFields, LocalCounter, OriginalBasis, PendingRecord, Preparation,
        preparation::prepare_change, receive_accepted,
    },
    record::{AcceptedRecord, Observation, RecordId, RecordKey, RecordState, RecordValue},
};

/// Construct checked independent successful results, retaining their actual tuple.
fn values(second: i64, other_key: bool) -> anyhow::Result<DirectoryRecord> {
    let mut record = DirectoryRecord::new(id(10));
    record.record(DirectorySnapshot::new(DirectorySnapshotFields {
        provider: if second > 10 {
            DirectoryProvider::HamQth
        } else {
            DirectoryProvider::Qrz
        },
        key: LookupKey::from_recorded(&RecordedCallsign::new("K1ABC")?),
        lookup_time: UtcTimestamp::new(second, 0)?,
        lookup_id: id(second as u128),
        facts: DirectoryFacts {
            name: (second < 30).then(|| format!("Lookup {second}")),
            ..Default::default()
        },
    })?)?;
    if other_key {
        record.record(DirectorySnapshot::new(DirectorySnapshotFields {
            provider: DirectoryProvider::Qrz,
            key: LookupKey::from_recorded(&RecordedCallsign::new("K2ABC")?),
            lookup_time: UtcTimestamp::new(12, 0)?,
            lookup_id: id(100),
            facts: Default::default(),
        })?)?;
    }
    Ok(record)
}

/// Use existing real accepted-context fixtures rather than inventing arrival timestamps.
fn accepted(revision: u64, values: DirectoryRecord) -> anyhow::Result<AcceptedRecord> {
    AcceptedRecord::new(
        RecordKey {
            account: id(1),
            id: RecordId::Directory(id(10)),
        },
        RecordState::Live(RecordValue::Directory(Box::new(values))),
        support::accepted(revision, "Context fixture").stamp,
    )
}

#[test]
fn directory_refreshes_combine_without_qso_review() -> anyhow::Result<()> {
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let definitions = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &definitions,
    };
    let original = accepted(1, values(10, false)?)?;
    let pending: ClientRecord = ClientRecordFields {
        key: original.key(),
        latest: Observation::Accepted(original.clone()),
        retained: vec![],
        pending: Some(PendingRecord {
            key: original.key(),
            original: OriginalBasis::Accepted(original),
            working: RecordState::Live(RecordValue::Directory(Box::new(values(11, false)?))),
            prepared: None,
            witness: None,
        }),
    }
    .try_into()?;
    let Preparation::Ready {
        mut record,
        counter,
        change,
    } = prepare_change(
        &pending,
        id(4),
        &LocalCounter {
            actor: id(4),
            high_water: 0,
        },
        &[],
        &evidence,
    )?
    else {
        panic!("ready")
    };
    support::edit_client(&mut record, |fields| {
        fields.pending.as_mut().unwrap().working =
            RecordState::Live(RecordValue::Directory(Box::new(values(20, true).unwrap())));
    });
    let mut receipt = accepted(2, values(11, false)?)?;
    support::edit_stamp(&mut receipt.stamp, |stamp| {
        stamp.vector = stamp.vector.join(change.vector());
        Ok(())
    });
    let received = receive_accepted(&record, &receipt, &evidence)?;
    assert!(received.review.is_none());
    assert_eq!(
        received.record.working_state(),
        Some(&RecordState::Live(RecordValue::Directory(Box::new(
            values(20, true)?
        ))))
    );
    let mut newer = accepted(3, values(30, false)?)?;
    support::edit_stamp(&mut newer.stamp, |stamp| {
        stamp.vector = stamp.vector.join(change.vector());
        Ok(())
    });
    let received = receive_accepted(&received.record, &newer, &evidence)?;
    assert!(
        received.review.is_none(),
        "independent provider refreshes never require QSO review"
    );
    let Some(RecordState::Live(RecordValue::Directory(combined))) = received.record.working_state()
    else {
        panic!("directory")
    };
    assert_eq!(combined.snapshots().count(), 2);
    let winner = combined
        .snapshot(&LookupKey::from_recorded(&RecordedCallsign::new("K1ABC")?))
        .unwrap();
    assert_eq!(winner.lookup_id(), id(30));
    assert!(
        winner.facts().name.is_none(),
        "sparse success replaces the complete prior winner"
    );
    assert!(matches!(
        prepare_change(&received.record, id(4), &counter, &[], &evidence)?,
        Preparation::Ready { .. }
    ));
    let reverse = clacks_sync::directory::combine(&values(30, false)?, &values(20, true)?)?;
    assert_eq!(&reverse, combined.as_ref());
    Ok(())
}

#[test]
fn directory_history_for_deleted_parent_does_not_block_completion() -> anyhow::Result<()> {
    use clacks_sync::{
        acceptance::{
            AcceptanceDecision, AcceptanceInput, RejectionReason, ServerChange,
            plan_server_acceptance,
        },
        client::VersionCondition,
        membership::{ClientMembership, MembershipSnapshot},
        record::Position,
        recovery::{RecoveredDecision, plan_recovered_install},
    };
    let (mut account, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 10;
    let definitions = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &definitions,
    };
    let members = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![ClientMembership {
            actor: id(4),
            created_at: Position {
                timeline: id(2),
                revision: 0,
            },
            closed_at: None,
            acknowledged: None,
        }],
    };
    let parent = support::accepted(1, "Original parent");
    let deleted = Observation::Accepted(AcceptedRecord::new(
        parent.key(),
        RecordState::Deleted,
        support::accepted(2, "Deleted parent").stamp,
    )?);
    let incoming = accepted(3, values(10, false)?)?;
    let current = Observation::Absent {
        key: incoming.key(),
        checkpoint: clacks_sync::account::InstalledCheckpoint {
            account: id(1),
            coverage: Default::default(),
            baseline: id(3),
        },
    };
    let refs = [deleted.clone()];
    let waiting = clacks_sync::transfer::readiness::plan_readiness(
        &clacks_sync::transfer::readiness::ReadinessInput {
            account: id(1),
            candidates: std::slice::from_ref(&incoming),
            heads: &[],
            support: &[],
            units: &[],
        },
        &evidence,
    )?;
    assert!(
        waiting.ready.is_empty(),
        "directory history must stage until actual parent evidence arrives"
    );
    let ready = clacks_sync::transfer::readiness::plan_readiness(
        &clacks_sync::transfer::readiness::ReadinessInput {
            account: id(1),
            candidates: std::slice::from_ref(&incoming),
            heads: &[],
            support: match &deleted {
                Observation::Accepted(value) => std::slice::from_ref(value),
                _ => unreachable!(),
            },
            units: &[],
        },
        &evidence,
    )?;
    assert!(
        ready.staged.is_empty(),
        "a retained parent tombstone suffices for inert history"
    );
    let input = AcceptanceInput {
        destination_registry: None,
        bindings: &[],
        account: &account,
        membership: &members,
        current: &current,
        references: &refs,
    };
    let proposal = ServerChange {
        key: incoming.key(),
        state: incoming.state().clone(),
        expected: VersionCondition::from_observation(&current)?,
        dependencies: vec![VersionCondition::from_observation(&deleted)?],
    };
    assert!(
        matches!(plan_server_acceptance(&proposal,&input,&evidence)?,AcceptanceDecision::Rejected(RejectionReason::DeletedDirectoryParent { qso }) if qso==id(10))
    );
    assert!(
        matches!(
            plan_recovered_install(&incoming, None, id(4), &input, &evidence)?,
            RecoveredDecision::Install { .. }
        ),
        "previously accepted history with a known deleted parent remains installable and inert"
    );
    let missing = AcceptanceInput {
        references: &[],
        ..input
    };
    assert!(
        !matches!(
            plan_recovered_install(&incoming, None, id(4), &missing, &evidence)?,
            RecoveredDecision::Install { .. } | RecoveredDecision::Covered
        ),
        "missing parent evidence cannot become empty successful completion"
    );
    Ok(())
}

#[test]
fn directory_causal_successors_and_recovery_keep_lookup_winners() -> anyhow::Result<()> {
    use clacks_sync::{
        acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange, plan_server_acceptance},
        client::VersionCondition,
        membership::{ClientMembership, MembershipSnapshot},
        record::Position,
        recovery::{RecoveredDecision, plan_recovered_install},
    };
    let (mut account, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 10;
    let definitions = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &definitions,
    };
    let members = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![ClientMembership {
            actor: id(4),
            created_at: Position {
                timeline: id(2),
                revision: 0,
            },
            closed_at: None,
            acknowledged: None,
        }],
    };
    let parent = Observation::Accepted(support::accepted(1, "Live parent"));
    let current = Observation::Accepted(accepted(3, values(30, true)?)?);
    let incoming = accepted(4, values(10, false)?)?;
    let refs = [parent.clone()];
    let input = AcceptanceInput {
        destination_registry: None,
        bindings: &[],
        account: &account,
        membership: &members,
        current: &current,
        references: &refs,
    };
    let proposal = ServerChange {
        key: incoming.key(),
        state: incoming.state().clone(),
        expected: VersionCondition::from_observation(&current)?,
        dependencies: vec![VersionCondition::from_observation(&parent)?],
    };
    let AcceptanceDecision::Commit(committed) =
        plan_server_acceptance(&proposal, &input, &evidence)?
    else {
        panic!("commit")
    };
    assert_eq!(
        committed.accepted.state(),
        match &current {
            Observation::Accepted(value) => value.state(),
            _ => panic!("accepted directory"),
        }
    );
    let RecoveredDecision::Reconciled(recovered) =
        plan_recovered_install(&incoming, None, id(4), &input, &evidence)?
    else {
        panic!("reconciled directory history")
    };
    assert_eq!(
        recovered.accepted.state(),
        match &current {
            Observation::Accepted(value) => value.state(),
            _ => panic!("accepted directory"),
        }
    );
    Ok(())
}
