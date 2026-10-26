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
    let captured = support::telemetry::capture_filtered(
        || plan_recovery_fork(id(5), &a, &m, &a.installed),
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    let span = captured.span("sync.recovery.fork.plan");
    assert_eq!(span.fields.get("level").map(String::as_str), Some("INFO"));
    assert_eq!(span.fields.get("account_id"), Some(&a.account.to_string()));
    assert_eq!(
        span.fields.get("new_timeline_id"),
        Some(&id::<clacks_model::ids::TimelineId>(5).to_string())
    );
    assert_eq!(
        span.fields.get("restored_revision").map(String::as_str),
        Some("160")
    );
    let fork = captured.result.unwrap();
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
        None,
        id(4),
        &clacks_sync::acceptance::AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &fork.next_account,
            membership: &m,
            current: &current,
            references: &[],
        },
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
            None,
            id(4),
            &clacks_sync::acceptance::AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &fork.next_account,
                membership: &m,
                current: &current,
                references: &[]
            },
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
        None,
        id(4),
        &clacks_sync::acceptance::AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &m,
            current: &Observation::Accepted(competing),
            references: &[],
        },
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

#[test]
fn sparse_certified_absence_uses_ancestry_at_the_checkpoint_cut() {
    use clacks_sync::lineage::{BoundaryKind, TimelineBoundary};
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let mut lineage = b.lineage.clone();
    lineage
        .add_boundary(TimelineBoundary {
            at: Position {
                timeline: id(5),
                revision: 0,
            },
            includes: Coverage::from_entries([(id(2), 200)]).unwrap(),
            kind: BoundaryKind::ForkOrigin,
        })
        .unwrap();
    account.head = Position {
        timeline: id(5),
        revision: 50,
    };
    account.installed = Coverage::from(account.head);
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let incoming = accepted(180, "old");
    let absent = Observation::Absent {
        key: incoming.key,
        checkpoint: InstalledCheckpoint {
            account: id(1),
            baseline: id(3),
            coverage: account.installed.clone(),
        },
    };
    assert!(
        plan_recovered_install(
            &incoming,
            None,
            id(4),
            &clacks_sync::acceptance::AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &account,
                membership: &membership(),
                current: &absent,
                references: &[]
            },
            &ev
        )
        .is_err()
    );
}

#[test]
fn unknown_order_never_approves_differing_values_and_unavailable_is_two_way() {
    let mut returned_reason = None;
    let captured = support::telemetry::capture(|| {
        use clacks_sync::{
            client::{OriginalBasis, ReviewReason},
            clock::{ClockActor, RecordVector},
            lineage::{BoundaryKind, TimelineBoundary},
        };
        let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
        let mut other = b.clone();
        other.id = id(20);
        let defs = [b.clone(), other];
        let mut lineage = b.lineage.clone();
        lineage
            .add_boundary(TimelineBoundary {
                at: Position {
                    timeline: id(5),
                    revision: 0,
                },
                includes: Coverage::from_entries([(id(2), 0)]).unwrap(),
                kind: BoundaryKind::ForkOrigin,
            })
            .unwrap();
        account.head = Position {
            timeline: id(5),
            revision: 50,
        };
        let ev = EvidenceView {
            lineage: &lineage,
            baselines: &defs,
        };
        let mut incoming = accepted(10, "incoming");
        incoming.stamp.baseline = id(20);
        let mut shared = accepted(5, "original");
        shared.stamp.accepted_at.timeline = id(5);
        shared.stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(5)), 5)]).unwrap();
        let shared = Observation::Accepted(shared);
        let original = OriginalBasis::Accepted(accepted(1, "original"));
        let input = clacks_sync::acceptance::AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &membership(),
            current: &shared,
            references: &[],
        };
        let RecoveredDecision::Review(review) =
            plan_recovered_install(&incoming, Some(&original), id(4), &input, &ev).unwrap()
        else {
            panic!("unknown ordering review")
        };
        assert!(matches!(review.reason, ReviewReason::OrderingUnknown(_)));
        returned_reason = Some(review.reason.clone());
        incoming.stamp.baseline = id(3);
        let missing = OriginalBasis::Unavailable {
            key: incoming.key,
            reason: clacks_sync::error::UnknownReason::MissingOriginal,
        };
        let RecoveredDecision::Review(review) =
            plan_recovered_install(&incoming, Some(&missing), id(4), &input, &ev).unwrap()
        else {
            panic!("two-way review")
        };
        assert_eq!(review.reason, ReviewReason::MissingOriginal);
    });
    let event = captured.event("recovery.review_required");
    assert_eq!(
        event.fields.get("review_reason"),
        Some(&format!("{:?}", returned_reason.unwrap()))
    );
    captured.result
}

#[test]
fn tagged_qso_and_equipment_profile_context_joins_have_reference_guards() {
    use clacks_sync::{
        clock::{ClockActor, RecordVector},
        lineage::{BoundaryKind, TimelineBoundary},
        record::{RecordId, RecordKey, RecordValue},
    };
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let mut lineage = b.lineage.clone();
    lineage
        .add_boundary(TimelineBoundary {
            at: Position {
                timeline: id(5),
                revision: 0,
            },
            includes: Coverage::from_entries([(id(2), 0)]).unwrap(),
            kind: BoundaryKind::ForkOrigin,
        })
        .unwrap();
    account.head = Position {
        timeline: id(5),
        revision: 50,
    };
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let mut q = accepted(10, "same");
    if let RecordState::Live(RecordValue::Qso(q)) = &mut q.state {
        q.tags.insert(id(30));
    }
    let mut tag = accepted(10, "tag");
    tag.key = RecordKey {
        account: id(1),
        id: RecordId::Tag(id(30)),
    };
    tag.state = RecordState::Live(RecordValue::Tag(Box::new(
        clacks_model::tag::LogbookTag::new(id(30), "tag".into()),
    )));
    let mut profile = accepted(10, "profile");
    profile.key = RecordKey {
        account: id(1),
        id: RecordId::Profile(id(40)),
    };
    profile.state = RecordState::Live(RecordValue::Profile(Box::new(
        clacks_model::profile::SavedLocationProfile {
            id: id(40),
            name: "profile".into(),
            station: support::qso(id(10)).own_station,
            default_equipment: Some(id(50)),
        },
    )));
    let mut equipment = accepted(10, "equipment");
    equipment.key = RecordKey {
        account: id(1),
        id: RecordId::Equipment(id(50)),
    };
    equipment.state = RecordState::Live(RecordValue::Equipment(Box::new(
        clacks_model::profile::EquipmentConfiguration {
            id: id(50),
            name: "rig".into(),
            values: clacks_model::equipment::EquipmentValues {
                equipment_description: Some("rig".into()),
                ..Default::default()
            },
        },
    )));
    for (incoming, reference) in [(q, tag), (profile, equipment)] {
        let mut shared = incoming.clone();
        shared.stamp.accepted_at = Position {
            timeline: id(5),
            revision: 5,
        };
        shared.stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(5)), 5)]).unwrap();
        let shared = Observation::Accepted(shared);
        let refs = [Observation::Accepted(reference.clone())];
        let input = clacks_sync::acceptance::AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &membership(),
            current: &shared,
            references: &refs,
        };
        let RecoveredDecision::Reconciled(commit) =
            plan_recovered_install(&incoming, None, id(4), &input, &ev).unwrap()
        else {
            panic!("equal reference-bearing values reconcile")
        };
        assert_eq!(commit.dependencies.len(), 1);
        assert_eq!(commit.dependencies.first().unwrap().key(), reference.key);
        let missing = clacks_sync::acceptance::AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            references: &[],
            ..input
        };
        let RecoveredDecision::Review(review) =
            plan_recovered_install(&incoming, None, id(4), &missing, &ev).unwrap()
        else {
            panic!("missing reference review")
        };
        assert!(matches!(
            review.reason,
            clacks_sync::client::ReviewReason::MissingReference(_)
        ));
    }
}
