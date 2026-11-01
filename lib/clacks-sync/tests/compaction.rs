mod installation;

use crate::support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{OriginalBasis, PendingRecord},
    compaction::seal_basis,
    lineage::Coverage,
};
use support::{accepted, id};

fn checkpoint(revision: u64) -> InstalledCheckpoint {
    InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: Coverage::from_entries([(id(2), revision)]).unwrap(),
    }
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
