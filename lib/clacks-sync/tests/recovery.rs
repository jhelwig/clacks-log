mod support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    lineage::Coverage,
    membership::{ClientMembership, MembershipSnapshot},
    record::{Observation, Position, RecordState},
    recovery::{
        HistoryCheck, HistoryObservation, RecoveredDecision, check_history,
        plan_recovered_coverage, plan_recovered_install, plan_recovery_fork,
    },
};
use support::{accepted, id};

fn membership() -> MembershipSnapshot {
    MembershipSnapshot {
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
    }
}

#[test]
fn rollback_fork_only_covers_completed_old_windows() {
    let (mut a, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    a.head.revision = 160;
    a.installed = Coverage::from_entries([(id(2), 160)]).unwrap();
    let defs = [b.clone()];
    let m = membership();
    let mut lineage = b.lineage.clone();
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let remembered = Coverage::from_entries([(id(2), 200)]).unwrap();
    assert!(matches!(
        check_history(
            &HistoryObservation {
                account: id(1),
                advertised: a.head,
                remembered: remembered.clone()
            },
            &ev
        )
        .unwrap(),
        HistoryCheck::Regression { .. }
    ));
    let fork = plan_recovery_fork(id(5), &a, &m, &a.installed).unwrap();
    lineage.add_boundary(fork.boundary).unwrap();
    let mut restored = fork.next_account;
    restored.head.revision = 40;
    restored.installed = restored.installed.join(&Coverage::from(restored.head));
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let complete = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: remembered.clone(),
    };
    assert!(plan_recovered_coverage(&restored, &complete, &m, &ev).is_err());
    restored.installed = restored.installed.join(&remembered);
    let completed = plan_recovered_coverage(&restored, &complete, &m, &ev).unwrap();
    assert_eq!(completed.boundary.at.revision, 41);
    lineage.add_boundary(completed.boundary).unwrap();
    assert!(
        !lineage
            .coverage_at(Position {
                timeline: id(5),
                revision: 40
            })
            .unwrap()
            .covers(Position {
                timeline: id(2),
                revision: 199
            })
    );
    assert!(
        lineage
            .coverage_at(Position {
                timeline: id(5),
                revision: 41
            })
            .unwrap()
            .covers(Position {
                timeline: id(2),
                revision: 199
            })
    );
    assert!(
        !lineage
            .coverage_at(Position {
                timeline: id(5),
                revision: 41
            })
            .unwrap()
            .covers(Position {
                timeline: id(2),
                revision: 201
            })
    );
}

#[test]
fn retained_server_distributed_envelope_preserves_acceptance_and_deletion() {
    let (mut a, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    a.head.revision = 40;
    a.installed = Coverage::from_entries([(id(2), 40)]).unwrap();
    let m = membership();
    let fork = plan_recovery_fork(id(5), &a, &m, &a.installed).unwrap();
    let mut lineage = b.lineage.clone();
    lineage.add_boundary(fork.boundary).unwrap();
    let defs = [b];
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let mut retained = accepted(60, "accepted from A and distributed to C");
    retained.state = RecordState::Deleted;
    let current = Observation::Accepted(accepted(30, "older backup"));
    let RecoveredDecision::Install {
        envelope, journal, ..
    } = plan_recovered_install(
        &retained,
        &current,
        None,
        id(4),
        &fork.next_account,
        &m,
        &ev,
    )
    .unwrap()
    else {
        panic!("retained install")
    };
    assert_eq!(envelope, retained);
    assert_eq!(journal.installed_at.timeline, id(5));
    assert_eq!(journal.stamp.accepted_at.timeline, id(2));
    let defs = [];
    let missing = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    assert!(matches!(
        plan_recovered_install(
            &retained,
            &current,
            None,
            id(4),
            &fork.next_account,
            &m,
            &missing
        )
        .unwrap(),
        RecoveredDecision::Review(_)
    ));
}

#[test]
fn ordinary_restart_and_overtaken_revision_are_not_proof_of_rollback_absence() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    for advertised in [200, 210] {
        assert_eq!(
            check_history(
                &HistoryObservation {
                    account: id(1),
                    advertised: Position {
                        timeline: id(2),
                        revision: advertised
                    },
                    remembered: Coverage::from_entries([(id(2), 200)]).unwrap()
                },
                &ev
            )
            .unwrap(),
            HistoryCheck::NoContradiction
        );
    }
}

#[test]
fn equal_divergent_values_join_context_without_business_review() {
    let (mut a, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    a.head.revision = 40;
    a.installed = Coverage::from_entries([(id(2), 40)]).unwrap();
    let m = membership();
    let fork = plan_recovery_fork(id(5), &a, &m, &a.installed).unwrap();
    let mut lineage = b.lineage.clone();
    lineage.add_boundary(fork.boundary).unwrap();
    let defs = [b];
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let mut account = fork.next_account;
    account.head.revision = 10;
    let incoming = accepted(60, "same");
    let mut competing = incoming.clone();
    competing.stamp.accepted_at = account.head;
    competing.stamp.vector = clacks_sync::clock::RecordVector::from_entries([(
        clacks_sync::clock::ClockActor::Server(id(5)),
        10,
    )])
    .unwrap();
    let RecoveredDecision::Reconciled(commit) = plan_recovered_install(
        &incoming,
        &Observation::Accepted(competing),
        None,
        id(4),
        &account,
        &m,
        &ev,
    )
    .unwrap() else {
        panic!("context join")
    };
    assert!(!commit.native_changed);
    assert_eq!(
        commit
            .accepted
            .stamp
            .vector
            .component(clacks_sync::clock::ClockActor::Server(id(2))),
        60
    );
}
