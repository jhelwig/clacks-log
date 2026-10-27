use crate::support;
use clacks_sync::{
    acceptance::{AcceptanceDecision, AcceptanceInput, plan_acceptance},
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{
        ClientRecord, LocalCounter, OriginalBasis, PendingRecord, Preparation, prepare_change,
        receive_accepted,
    },
    compaction::{CompactionInput, plan_baseline},
    compare::{RecordRelation, compare_records},
    lineage::Coverage,
    membership::{ClientMembership, MembershipSnapshot, revoke},
    record::{Observation, Position},
};
use std::collections::BTreeSet;
use support::{accepted, id};

fn members(ack: u64) -> MembershipSnapshot {
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
            acknowledged: Some(InstalledCheckpoint {
                account: id(1),
                baseline: id(3),
                coverage: Coverage::from_entries([(id(2), ack)]).unwrap(),
            }),
        }],
    }
}

#[test]
fn prepared_acceptance_lost_response_and_later_local_edit_compose() {
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 200;
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let m = members(200);
    let a = accepted(200, "A");
    let c = ClientRecord {
        retained: Vec::new(),
        key: a.key,
        latest: Observation::Accepted(a.clone()),
        pending: Some(PendingRecord {
            witness: None,
            key: a.key,
            original: OriginalBasis::Accepted(a.clone()),
            working: accepted(200, "B").state,
            prepared: None,
        }),
    };
    let Preparation::Ready {
        mut record,
        change,
        counter,
    } = prepare_change(
        &c,
        id(4),
        &LocalCounter {
            actor: id(4),
            high_water: 0,
        },
        &[],
        &ev,
    )
    .unwrap()
    else {
        panic!("ready")
    };
    let old = Observation::Accepted(a);
    let AcceptanceDecision::Commit(commit) = plan_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &m,
            current: &old,
            references: &[],
        },
        &ev,
    )
    .unwrap() else {
        panic!("accepted")
    };
    record.pending.as_mut().unwrap().working = accepted(200, "C").state;
    let reloaded: ClientRecord =
        serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
    let update = receive_accepted(&reloaded, &commit.accepted, &ev).unwrap();
    assert!(update.review.is_none());
    let Preparation::Ready { change: later, .. } =
        prepare_change(&update.record, id(4), &counter, &[], &ev).unwrap()
    else {
        panic!("later change")
    };
    assert_eq!(
        later
            .vector
            .component(clacks_sync::clock::ClockActor::Client(id(4))),
        2
    );
    assert!(matches!(
        plan_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &commit.next_account,
                membership: &m,
                current: &Observation::Accepted(commit.accepted),
                references: &[]
            },
            &ev
        )
        .unwrap(),
        AcceptanceDecision::Covered(_)
    ));
}

#[test]
fn revocation_and_projection_keep_post_boundary_record_order_interpretable() {
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 80;
    account.installed = Coverage::from(account.head);
    let m = members(80);
    let closed = revoke(id(4), &account, &m).unwrap();
    let mut account = closed.next_account;
    account.head.revision = 950;
    account.installed = Coverage::from(account.head);
    let mut m = closed.next;
    m.clients.push(ClientMembership {
        actor: id(5),
        created_at: Position {
            timeline: id(2),
            revision: 82,
        },
        closed_at: None,
        acknowledged: Some(InstalledCheckpoint {
            account: id(1),
            baseline: id(3),
            coverage: Coverage::from_entries([(id(2), 300)]).unwrap(),
        }),
    });
    let mut old = accepted(400, "old");
    old.stamp.vector = old.stamp.vector.join(
        &clacks_sync::clock::RecordVector::from_entries([(
            clacks_sync::clock::ClockActor::Client(id(4)),
            1,
        )])
        .unwrap(),
    );
    let mut latest = accepted(600, "latest");
    latest.stamp.vector = latest.stamp.vector.join(&old.stamp.vector);
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let plan = plan_baseline(
        id(20),
        &CompactionInput {
            account: &account,
            membership: &m,
            records: &[latest],
            surviving: &[],
        },
        &ev,
    )
    .unwrap();
    let projected = plan.projected.first().unwrap();
    let defs = [b.clone(), plan.baseline.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    assert_eq!(
        compare_records(&old, projected, &ev).unwrap(),
        RecordRelation::Before
    );
}

#[test]
fn partial_install_reload_completion_and_recovery_ancestry_compose() {
    use clacks_sync::{
        recovery::{
            RecoveredDecision, plan_recovered_coverage, plan_recovered_install, plan_recovery_fork,
        },
        transfer::{
            RecordRequirement, WindowManifest, WindowMode, WindowProgress, finish_window,
            plan_batch,
        },
    };
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 160;
    account.installed = Coverage::from(account.head);
    let m = members(160);
    let defs = [b.clone()];
    let mut lineage = b.lineage.clone();
    let fork = plan_recovery_fork(id(5), &account, &m, &account.installed).unwrap();
    lineage.add_boundary(fork.boundary).unwrap();
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let retained = accepted(200, "retained");
    let old = Observation::Accepted(accepted(150, "backup"));
    let RecoveredDecision::Install {
        envelope,
        next_account,
        ..
    } = plan_recovered_install(
        &retained,
        None,
        id(4),
        &clacks_sync::acceptance::AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &fork.next_account,
            membership: &m,
            current: &old,
            references: &[],
        },
        &ev,
    )
    .unwrap()
    else {
        panic!("install")
    };
    let mut account = next_account;
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: Coverage::from_entries([(id(2), 200)]).unwrap(),
    };
    let manifest = WindowManifest::new(
        id(1),
        source.clone(),
        WindowMode::Full,
        vec![RecordRequirement {
            key: retained.key,
            stamp: retained.stamp.clone(),
            deleted: false,
        }],
        vec![],
        defs.to_vec(),
        vec![],
    )
    .unwrap();
    let progress = WindowProgress {
        source,
        mode: WindowMode::Full,
        generation: 0,
        installed: Default::default(),
        staged: Vec::new(),
        completed_units: BTreeSet::new(),
    };
    let previous = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: account.installed.clone(),
    };
    assert!(finish_window(&manifest, &progress, &[], &previous, &ev).is_err());
    let batch = plan_batch(&manifest, &progress, &[envelope], &[], &ev).unwrap();
    let progress = WindowProgress {
        generation: batch.next_generation,
        installed: batch.installed_keys.into_iter().collect(),
        staged: Vec::new(),
        completed_units: BTreeSet::new(),
        ..progress
    };
    let progress: WindowProgress =
        serde_json::from_str(&serde_json::to_string(&progress).unwrap()).unwrap();
    let completion = finish_window(&manifest, &progress, &batch.accepted, &previous, &ev).unwrap();
    let committed: InstalledCheckpoint =
        serde_json::from_str(&serde_json::to_string(&completion.next).unwrap()).unwrap();
    account.installed = committed.coverage.clone();
    let boundary = plan_recovered_coverage(&account, &committed, &m, &ev).unwrap();
    lineage.add_boundary(boundary.boundary).unwrap();
    assert!(
        lineage
            .coverage_at(boundary.next_account.head)
            .unwrap()
            .covers(Position {
                timeline: id(2),
                revision: 199
            })
    );
    assert!(
        !lineage
            .coverage_at(boundary.next_account.head)
            .unwrap()
            .covers(Position {
                timeline: id(2),
                revision: 201
            })
    );
}
