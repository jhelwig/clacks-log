use crate::support;
use clacks_sync::{
    acceptance::{
        AcceptanceDecision, AcceptanceInput, ServerChange, plan_acceptance, plan_server_acceptance,
    },
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{PreparedChange, VersionCondition},
    clock::ClockActor,
    membership::{ClientMembership, MembershipSnapshot},
    record::{Observation, Position, RecordId, RecordKey, RecordState, RecordValue},
};
use support::{accepted, id};

#[test]
fn accepts_once_with_atomic_conditions_and_rejects_closed_actor() {
    let captured = support::telemetry::capture(|| {
        let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
        account.head.revision = 200;
        let defs = [b.clone()];
        let ev = EvidenceView {
            lineage: &b.lineage,
            baselines: &defs,
        };
        let mut members = MembershipSnapshot {
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
        let a = accepted(200, "A");
        let current = Observation::Accepted(a.clone());
        let change: PreparedChange = clacks_sync::client::PreparedChangeFields {
            key: a.key(),
            state: accepted(200, "B").state().clone(),
            vector: a
                .stamp
                .vector()
                .increment(ClockActor::Client(id(4)), 0)
                .unwrap()
                .0,
            actor: id(4),
            expected: VersionCondition::from_observation(&current).unwrap(),
            dependencies: vec![],
        }
        .try_into()
        .expect("valid prepared fixture");
        let AcceptanceDecision::Commit(commit) = plan_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &account,
                membership: &members,
                current: &current,
                references: &[],
            },
            &ev,
        )
        .unwrap() else {
            panic!("commit")
        };
        assert_eq!(commit.accepted.stamp.accepted_at().revision, 201);
        assert_eq!(
            commit
                .accepted
                .stamp
                .vector()
                .component(ClockActor::Client(id(4))),
            1
        );
        assert!(commit.native_changed);
        assert!(!commit.journal.requirement.deleted);
        assert_eq!(commit.journal.requirement.stamp, commit.accepted.stamp);
        assert_eq!(commit.expected_head, account.head);
        assert_eq!(commit.expected_membership_generation, 0);
        let next = Observation::Accepted(commit.accepted.clone());
        assert!(matches!(
            plan_acceptance(
                &change,
                &AcceptanceInput {
                    destination_registry: None,
                    bindings: &[],
                    account: &commit.next_account,
                    membership: &members,
                    current: &next,
                    references: &[]
                },
                &ev
            )
            .unwrap(),
            AcceptanceDecision::Covered(_)
        ));
        members.clients.first_mut().unwrap().closed_at = Some(Position {
            timeline: id(2),
            revision: 202,
        });
        assert!(matches!(
            plan_acceptance(
                &change,
                &AcceptanceInput {
                    destination_registry: None,
                    bindings: &[],
                    account: &account,
                    membership: &members,
                    current: &current,
                    references: &[]
                },
                &ev
            )
            .unwrap(),
            AcceptanceDecision::Rejected(_)
        ));
    });
    let outcomes: Vec<_> = captured
        .observations
        .events
        .iter()
        .filter(|s| {
            s.fields
                .get("event")
                .is_some_and(|v| v == "acceptance.decided")
                && s.fields.get("origin").is_some_and(|v| v == "client")
        })
        .map(|s| s.fields.get("decision").unwrap().as_str())
        .collect();
    assert_eq!(outcomes, vec!["proposed", "covered", "rejected"]);
    let rejected = captured.event("actor.admission_denied");
    assert_eq!(
        rejected.fields.get("reason").map(String::as_str),
        Some("actor_closed")
    );
    assert_eq!(
        rejected.fields.get("actor_id"),
        Some(&id::<clacks_model::ids::ClientActorId>(4).to_string())
    );
    assert_eq!(
        rejected.fields.get("closed_revision").map(String::as_str),
        Some("202")
    );
    captured.result
}

#[test]
fn empty_account_seeds_groups_without_a_client_actor() {
    let (account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let members = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![],
    };
    let groups: Vec<clacks_model::group::GroupDefinition> =
        serde_json::from_str(include_str!("support/definitions.json")).unwrap();
    let group = groups.first().unwrap().clone();
    let key = RecordKey {
        account: id(1),
        id: RecordId::Group(group.id()),
    };
    let absent = Observation::Absent {
        key,
        checkpoint: InstalledCheckpoint {
            account: id(1),
            baseline: id(3),
            coverage: b.coverage.clone(),
        },
    };
    let change = ServerChange {
        key,
        state: RecordState::Live(RecordValue::Group(Box::new(group))),
        expected: VersionCondition::from_observation(&absent).unwrap(),
        dependencies: vec![],
    };
    let AcceptanceDecision::Commit(commit) = plan_server_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &members,
            current: &absent,
            references: &[],
        },
        &ev,
    )
    .unwrap() else {
        panic!("seed")
    };
    assert_eq!(
        commit.accepted.stamp.vector().entries().collect::<Vec<_>>(),
        vec![(ClockActor::Server(id(2)), 1)]
    );
}

#[test]
fn missing_tag_rejects_without_changing_local_values() {
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 200;
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let members = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![],
    };
    let a = accepted(200, "A");
    let current = Observation::Accepted(a.clone());
    let mut state = a.state().clone();
    if let RecordState::Live(RecordValue::Qso(q)) = &mut state {
        support::edit(q, |q| {
            q.tags.insert(id(8));
            Ok(())
        });
    }
    let change = ServerChange {
        key: a.key(),
        state: state.clone(),
        expected: VersionCondition::from_observation(&current).unwrap(),
        dependencies: vec![],
    };
    assert!(matches!(
        plan_server_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &account,
                membership: &members,
                current: &current,
                references: &[]
            },
            &ev
        )
        .unwrap(),
        AcceptanceDecision::Rejected(_)
    ));
    assert_eq!(change.state, state);
}

#[test]
fn context_only_acceptance_is_not_a_native_change_and_guards_epochs() {
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 200;
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let mut members = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![],
    };
    let a = accepted(200, "A");
    let current = Observation::Accepted(a.clone());
    let change = ServerChange {
        key: a.key(),
        state: a.state().clone(),
        expected: VersionCondition::from_observation(&current).unwrap(),
        dependencies: vec![],
    };
    let AcceptanceDecision::Commit(commit) = plan_server_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &members,
            current: &current,
            references: &[],
        },
        &ev,
    )
    .unwrap() else {
        panic!("metadata commit")
    };
    assert!(!commit.native_changed);
    account.head.revision = 210;
    let AcceptanceDecision::Commit(later) = plan_server_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &members,
            current: &current,
            references: &[],
        },
        &ev,
    )
    .unwrap() else {
        panic!("fresh plan")
    };
    assert_ne!(commit.expected_head, later.expected_head);
    members.generation = 1;
    assert!(
        plan_server_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &account,
                membership: &members,
                current: &current,
                references: &[]
            },
            &ev
        )
        .is_err()
    );
}

#[test]
fn stale_tag_dependency_and_invalid_group_are_diagnostic() {
    let captured = support::telemetry::capture(|| {
        let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
        account.head.revision = 200;
        let defs = [b.clone()];
        let ev = EvidenceView {
            lineage: &b.lineage,
            baselines: &defs,
        };
        let members = MembershipSnapshot {
            account: id(1),
            generation: 0,
            clients: vec![],
        };
        let a = accepted(200, "A");
        let current = Observation::Accepted(a.clone());
        let key = RecordKey {
            account: id(1),
            id: RecordId::Tag(id(8)),
        };
        let tag = clacks_sync::record::AcceptedRecord::new(
            key,
            RecordState::Live(RecordValue::Tag(Box::new(
                clacks_model::tag::LogbookTag::new(id(8), "tag".into()),
            ))),
            a.stamp.clone(),
        )
        .unwrap();
        let mut state = a.state().clone();
        if let RecordState::Live(RecordValue::Qso(q)) = &mut state {
            support::edit(q, |q| {
                q.tags.insert(id(8));
                Ok(())
            });
        }
        let dep = VersionCondition::from_observation(&Observation::Accepted(tag.clone())).unwrap();
        let mut deleted = tag;

        support::edit_accepted(&mut deleted, |deleted| {
            deleted.state = RecordState::Deleted;
        });
        deleted.stamp = accepted(201, "x").stamp;
        let refs = [Observation::Accepted(deleted)];
        let change = ServerChange {
            key: a.key(),
            state,
            expected: VersionCondition::from_observation(&current).unwrap(),
            dependencies: vec![dep],
        };
        assert!(matches!(
            plan_server_acceptance(
                &change,
                &AcceptanceInput {
                    destination_registry: None,
                    bindings: &[],
                    account: &account,
                    membership: &members,
                    current: &current,
                    references: &refs
                },
                &ev
            )
            .unwrap(),
            AcceptanceDecision::Stale(_)
        ));
    });

    let span = captured.event("acceptance.decided");
    assert_eq!(
        span.fields.get("decision").map(String::as_str),
        Some("stale")
    );
    assert!(!span.fields.contains_key("error_class"));
    let counter = captured
        .metrics
        .iter()
        .find(|m| {
            m.name == "clacks_sync_acceptance_plans_total"
                && m.labels.get("origin").is_some_and(|v| v == "server")
                && m.labels.get("decision").is_some_and(|v| v == "stale")
        })
        .unwrap();
    assert_eq!(
        counter.value,
        metrics_util::debugging::DebugValue::Counter(1)
    );
    captured.result
}

#[test]
fn server_change_cannot_relabel_an_uninterpretable_current_baseline() {
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 200;
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let members = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![],
    };
    let mut a = accepted(200, "A");
    support::edit_stamp(&mut a.stamp, |stamp| {
        stamp.baseline = id(99);
        Ok(())
    });
    let current = Observation::Accepted(a.clone());
    let change = ServerChange {
        key: a.key(),
        state: a.state().clone(),
        expected: VersionCondition::from_observation(&current).unwrap(),
        dependencies: vec![],
    };
    assert!(
        plan_server_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &account,
                membership: &members,
                current: &current,
                references: &[]
            },
            &ev
        )
        .is_err()
    );
}
