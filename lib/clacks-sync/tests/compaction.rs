use crate::support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{OriginalBasis, PendingRecord},
    clock::{ClockActor, RecordVector},
    compaction::{CompactionInput, common_coverage, plan_baseline, seal_basis},
    lineage::Coverage,
    membership::{ClientMembership, MembershipSnapshot, revoke},
    record::Position,
};
use std::collections::BTreeSet;
use support::{accepted, id};

fn checkpoint(revision: u64) -> InstalledCheckpoint {
    InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: Coverage::from_entries([(id(2), revision)]).unwrap(),
    }
}

#[test]
fn oldest_authorized_ack_bounds_collection_until_revocation() {
    let (mut a, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    a.head.revision = 950;
    a.installed = checkpoint(950).coverage;
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let m = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: [900, 700, 300]
            .into_iter()
            .enumerate()
            .map(|(n, r)| ClientMembership {
                actor: id(n as u128 + 10),
                created_at: Position {
                    timeline: id(2),
                    revision: 0,
                },
                closed_at: None,
                acknowledged: Some(checkpoint(r)),
            })
            .collect(),
    };
    let captured = support::telemetry::capture(|| common_coverage(&checkpoint(950), &m, &ev));
    assert_eq!(captured.result.unwrap(), checkpoint(300).coverage);
    let limiting = captured
        .observations
        .events
        .iter()
        .find(|e| {
            e.fields.get("actor_id")
                == Some(&id::<clacks_model::ids::ClientActorId>(12).to_string())
        })
        .unwrap();
    assert_eq!(
        limiting
            .fields
            .get("acknowledged_revision")
            .map(String::as_str),
        Some("300")
    );
    let update = revoke(id(12), &a, &m).unwrap();
    assert_eq!(
        common_coverage(&checkpoint(951), &update.next, &ev).unwrap(),
        checkpoint(700).coverage
    );
    assert_eq!(update.next_account.membership_generation, 1);
    assert_eq!(update.expected_head.revision, 950);
}

#[test]
fn projection_removes_closed_actor_without_changing_originals_or_native_state() {
    let (mut a, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    a.head.revision = 950;
    a.installed = checkpoint(950).coverage;
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let closed = ClientMembership {
        actor: id(4),
        created_at: Position {
            timeline: id(2),
            revision: 0,
        },
        closed_at: Some(Position {
            timeline: id(2),
            revision: 80,
        }),
        acknowledged: None,
    };
    let active = ClientMembership {
        actor: id(5),
        created_at: Position {
            timeline: id(2),
            revision: 0,
        },
        closed_at: None,
        acknowledged: Some(checkpoint(300)),
    };
    let m = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![closed, active],
    };
    let mut shared = accepted(600, "shared");
    support::edit_stamp(&mut shared.stamp, |stamp| {
        stamp.vector = stamp
            .vector
            .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 3)]).unwrap());
        Ok(())
    });
    let records = [shared.clone()];
    let plan = plan_baseline(
        id(20),
        &CompactionInput {
            account: &a,
            membership: &m,
            records: &records,
            surviving: &[],
        },
        &ev,
    )
    .unwrap();
    assert!(plan.removable_clients.contains(&id(4)));
    let projected = plan.projected.first().unwrap();
    assert_eq!(
        projected
            .stamp
            .vector()
            .component(ClockActor::Client(id(4))),
        0
    );
    assert_eq!(projected.state(), shared.state());
    assert_eq!(projected.stamp.accepted_at(), shared.stamp.accepted_at());
    let client = clacks_sync::client::ClientRecordFields {
        key: shared.key(),
        latest: clacks_sync::record::Observation::Accepted(shared.clone()),
        pending: None,
        retained: vec![],
    }
    .try_into()
    .expect("coherent client fixture");
    let interpretations = [b.clone(), plan.baseline.clone()];
    let projected_view = EvidenceView {
        lineage: &b.lineage,
        baselines: &interpretations,
    };
    assert!(
        clacks_sync::client::receive_accepted(&client, projected, &projected_view)
            .unwrap()
            .review
            .is_none()
    );
    let source = checkpoint(950);
    let manifest = clacks_sync::transfer::WindowManifest::new(
        id(1),
        source.clone(),
        clacks_sync::transfer::WindowMode::Full,
        vec![clacks_sync::transfer::RecordRequirement {
            key: shared.key(),
            stamp: shared.stamp.clone(),
            deleted: false,
        }],
        vec![],
        interpretations.to_vec(),
        vec![],
    )
    .unwrap();
    let progress = clacks_sync::transfer::WindowProgress {
        source: source.clone(),
        mode: clacks_sync::transfer::WindowMode::Full,
        generation: 0,
        installed: Default::default(),
        staged: Vec::new(),
        completed_units: BTreeSet::new(),
    };
    assert!(
        clacks_sync::transfer::finish_window(
            &manifest,
            &progress,
            std::slice::from_ref(projected),
            &source,
            &projected_view
        )
        .is_ok()
    );
    let original = accepted(200, "original");
    let pending = PendingRecord {
        witness: None,
        key: original.key(),
        original: OriginalBasis::Accepted(original.clone()),
        working: accepted(200, "local").state().clone(),
        prepared: None,
    };
    let witness = seal_basis(&pending, &shared, &checkpoint(700), &ev).unwrap();
    assert_eq!(witness.original(), &OriginalBasis::Accepted(original));
    assert!(
        seal_basis(
            &pending,
            &shared,
            &checkpoint(700),
            &EvidenceView {
                lineage: &b.lineage,
                baselines: &[]
            }
        )
        .is_err()
    );
    let pinned = [shared.stamp.clone()];
    let pinned = plan_baseline(
        id(21),
        &CompactionInput {
            account: &a,
            membership: &m,
            records: &records,
            surviving: &pinned,
        },
        &ev,
    )
    .unwrap();
    assert!(!pinned.removable_clients.contains(&id(4)));
    let mut unavailable = shared.stamp.clone();
    support::edit_stamp(&mut unavailable, |stamp| {
        stamp.baseline = id(99);
        Ok(())
    });
    let captured = support::telemetry::capture(|| {
        plan_baseline(
            id(22),
            &CompactionInput {
                account: &a,
                membership: &m,
                records: &records,
                surviving: &[unavailable],
            },
            &ev,
        )
    });
    assert!(
        captured
            .result
            .as_ref()
            .unwrap()
            .removable_clients
            .is_empty()
    );
    assert_eq!(
        captured
            .event("compaction.collection_blocked")
            .fields
            .get("reason")
            .map(String::as_str),
        Some("surviving_baseline_unavailable")
    );
    let mut unavailable = shared.clone();
    support::edit_stamp(&mut unavailable.stamp, |stamp| {
        stamp.baseline = id(99);
        Ok(())
    });
    let captured = support::telemetry::capture(|| {
        plan_baseline(
            id(23),
            &CompactionInput {
                account: &a,
                membership: &m,
                records: &[unavailable],
                surviving: &[],
            },
            &ev,
        )
    });
    assert!(captured.result.as_ref().unwrap().projected.is_empty());
    assert_eq!(
        captured
            .event("compaction.projection_blocked")
            .fields
            .get("record_id"),
        Some(&shared.key().id.to_string())
    );
}

#[test]
fn witness_cannot_certify_an_observation_beyond_its_checkpoint() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let original = accepted(200, "original");
    let pending = PendingRecord {
        witness: None,
        key: original.key(),
        original: OriginalBasis::Accepted(original),
        working: accepted(200, "local").state().clone(),
        prepared: None,
    };
    assert!(seal_basis(&pending, &accepted(600, "shared"), &checkpoint(300), &ev).is_err());
}

#[test]
fn initializing_members_and_changed_rows_keep_projection_conditional() {
    let (mut a, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    a.head.revision = 950;
    a.installed = checkpoint(950).coverage;
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let mut m = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![ClientMembership {
            actor: id(4),
            created_at: Position {
                timeline: id(2),
                revision: 950,
            },
            closed_at: None,
            acknowledged: None,
        }],
    };
    assert_eq!(
        common_coverage(&checkpoint(950), &m, &ev).unwrap(),
        b.coverage
    );
    m.clients.first_mut().unwrap().acknowledged = Some(checkpoint(700));
    let rows = [accepted(600, "shared")];
    let plan = plan_baseline(
        id(20),
        &CompactionInput {
            account: &a,
            membership: &m,
            records: &rows,
            surviving: &[],
        },
        &ev,
    )
    .unwrap();
    assert!(!plan.expected_records.first().unwrap().matches(
        &clacks_sync::record::Observation::Accepted(accepted(601, "changed"))
    ));
    assert_eq!(plan.next_account.head.revision, 951);
    m.generation = 1;
    assert!(
        plan_baseline(
            id(20),
            &CompactionInput {
                account: &a,
                membership: &m,
                records: &rows,
                surviving: &[]
            },
            &ev
        )
        .is_err()
    );
}

#[test]
fn a_sealed_pending_basis_remains_usable_after_old_baseline_collection() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let original = accepted(200, "A");
    let shared = accepted(600, "A");
    let mut pending = PendingRecord {
        witness: None,
        key: original.key(),
        original: OriginalBasis::Accepted(original),
        working: accepted(200, "B").state().clone(),
        prepared: None,
    };
    let witness = seal_basis(&pending, &shared, &checkpoint(700), &ev).unwrap();
    pending.witness = Some(witness);
    let pending: PendingRecord =
        serde_json::from_str(&serde_json::to_string(&pending).unwrap()).unwrap();
    let mut new = b.clone();
    new.id = id(20);
    new.coverage = checkpoint(700).coverage;
    let defs = [new];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let mut shared = shared;
    support::edit_stamp(&mut shared.stamp, |stamp| {
        stamp.baseline = id(20);
        Ok(())
    });
    let client = clacks_sync::client::ClientRecordFields {
        retained: Vec::new(),
        key: shared.key(),
        latest: clacks_sync::record::Observation::Accepted(shared),
        pending: Some(pending),
    }
    .try_into()
    .expect("coherent client fixture");
    let mut future = accepted(960, "A");
    support::edit_stamp(&mut future.stamp, |stamp| {
        stamp.baseline = id(20);
        Ok(())
    });
    let update = clacks_sync::client::receive_accepted(&client, &future, &ev).unwrap();
    assert!(update.review.is_none());
    assert!(matches!(
        clacks_sync::client::preparation::prepare_change(
            &client,
            id(4),
            &clacks_sync::client::LocalCounter {
                actor: id(4),
                high_water: 0
            },
            &[],
            &ev
        )
        .unwrap(),
        clacks_sync::client::Preparation::Ready { .. }
    ));
}
