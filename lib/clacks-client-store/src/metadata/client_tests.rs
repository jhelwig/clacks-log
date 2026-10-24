use super::*;
use crate::{
    metadata::lineage,
    store::{ClientStore, StoreSettings},
};
use clacks_model::tag::LogbookTag;
use clacks_sync::{
    acceptance::{
        AcceptanceDecision, AcceptanceInput, ServerChange, plan_acceptance, plan_server_acceptance,
    },
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{OriginalBasis, Preparation, VersionCondition, prepare_change},
    compaction::seal_basis,
    error::UnknownReason,
    membership::{ClientMembership, MembershipSnapshot},
    record::{AcceptedRecord, Observation, RecordId, RecordState, RecordValue},
};

fn id<T: std::str::FromStr>(n: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(n).to_string().parse().unwrap()
}
fn tag(n: u128, name: &str) -> RecordState {
    RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
        id(n),
        name.into(),
    ))))
}

#[tokio::test]
async fn client_roles_reopen_with_distinct_original_working_and_frozen_values() -> anyhow::Result<()>
{
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let (account, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };
    let checkpoint = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: account.installed.clone(),
    };
    let key = RecordKey {
        account: id(1),
        id: RecordId::Tag(id(10)),
    };
    let absent = Observation::Absent { key, checkpoint };
    let membership = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![ClientMembership {
            actor: id(9),
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    let change = ServerChange {
        key,
        state: tag(10, "original"),
        expected: VersionCondition::from_observation(&absent)?,
        dependencies: vec![],
    };
    let input = AcceptanceInput {
        destination_registry: None,
        bindings: &[],
        account: &account,
        membership: &membership,
        current: &absent,
        references: &[],
    };
    let AcceptanceDecision::Commit(commit) = plan_server_acceptance(&change, &input, &evidence)?
    else {
        panic!("source acceptance expected")
    };
    let accepted = commit.accepted;
    let source_account = commit.next_account.clone();
    let checkpoint = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: commit.next_account.installed,
    };
    let mut pending = PendingRecord {
        key,
        original: OriginalBasis::Accepted(accepted.clone()),
        working: tag(10, "frozen"),
        prepared: None,
        witness: None,
    };
    pending.witness = Some(seal_basis(&pending, &accepted, &checkpoint, &evidence)?);
    let row = ClientRecord {
        key,
        latest: Observation::Accepted(accepted),
        retained: vec![],
        pending: Some(pending),
    };
    let counter = LocalCounter {
        actor: id(9),
        high_water: 0,
    };
    let Preparation::Ready {
        mut record,
        counter,
        ..
    } = prepare_change(&row, id(9), &counter, &[], &evidence)?
    else {
        panic!("frozen preparation expected")
    };
    record.pending.as_mut().unwrap().working = tag(10, "later working edit");
    let mut tx = store.write().await?;
    lineage::insert_baseline(&mut tx, &baseline).await?;
    lineage::set_lineage(&mut tx, &baseline.lineage).await?;
    lineage::set_checkpoint(&mut tx, &checkpoint).await?;
    super::store(&mut tx, &record).await?;
    store_counter(&mut tx, key, &counter).await?;
    assert_eq!(super::load(&mut tx, key).await?, Some(record.clone()));
    assert!(
        store_counter(
            &mut tx,
            key,
            &LocalCounter {
                actor: id(9),
                high_water: 0
            }
        )
        .await
        .is_err()
    );
    tx.commit().await?;
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    assert_eq!(crate::save::load(&store, key).await?, Some(record.clone()));
    let mut tx = store.write().await?;
    assert_eq!(super::counter(&mut tx, key, id(9)).await?, counter);
    let restored = lineage::interpretation(&mut tx, id(1)).await?.unwrap();
    let Preparation::Retry { change, .. } =
        prepare_change(&record, id(9), &counter, &[], &restored.view())?
    else {
        panic!("frozen retry expected")
    };
    assert_eq!(change.state, tag(10, "frozen"));
    let pending = record.pending.as_ref().unwrap();
    assert_eq!(pending.working, tag(10, "later working edit"));
    assert_eq!(pending.original.state(), Some(&tag(10, "original")));
    let input = AcceptanceInput {
        destination_registry: None,
        bindings: &[],
        account: &source_account,
        membership: &membership,
        current: &record.latest,
        references: &[],
    };
    let AcceptanceDecision::Commit(response) = plan_acceptance(&change, &input, &evidence)? else {
        panic!("client acceptance expected")
    };
    let response =
        clacks_sync::client::receive_accepted(&record, &response.accepted, &evidence)?.record;
    assert!(matches!(
        response.pending.as_ref().unwrap().original,
        OriginalBasis::SubsumedOwn { .. }
    ));
    super::store(&mut tx, &response).await?;
    assert_eq!(super::load(&mut tx, key).await?, Some(response.clone()));
    assert_eq!(
        lineage::current_checkpoint(&mut tx, id(1)).await?,
        Some(checkpoint.clone())
    );
    let mut forked = baseline.lineage.clone();
    forked.add_boundary(clacks_sync::lineage::TimelineBoundary {
        at: clacks_sync::record::Position {
            timeline: id(4),
            revision: 0,
        },
        includes: clacks_sync::lineage::Coverage::from(account.head),
        kind: clacks_sync::lineage::BoundaryKind::ForkOrigin,
    })?;
    lineage::set_lineage(&mut tx, &forked).await?;
    let alternative = AcceptedRecord::new(
        key,
        tag(10, "restored branch"),
        clacks_sync::record::VersionStamp {
            accepted_at: clacks_sync::record::Position {
                timeline: id(4),
                revision: 1,
            },
            baseline: id(3),
            vector: clacks_sync::clock::RecordVector::from_entries([(
                clacks_sync::clock::ClockActor::Server(id(4)),
                1,
            )])?,
        },
    )?;
    let fork_evidence = EvidenceView {
        lineage: &forked,
        baselines: &baselines,
    };
    let response =
        clacks_sync::client::receive_accepted(&response, &alternative, &fork_evidence)?.record;
    assert!(!response.retained.is_empty());
    super::store(&mut tx, &response).await?;
    assert_eq!(super::load(&mut tx, key).await?, Some(response.clone()));
    for (n, certified) in [(20, true), (21, false)] {
        let key = RecordKey {
            account: id(1),
            id: RecordId::Tag(id(n)),
        };
        let (latest, original) = if certified {
            (
                Observation::Absent {
                    key,
                    checkpoint: checkpoint.clone(),
                },
                OriginalBasis::NeverCreated {
                    key,
                    checkpoint: checkpoint.clone(),
                },
            )
        } else {
            (
                Observation::Unavailable {
                    key,
                    reason: UnknownReason::MissingOriginal,
                },
                OriginalBasis::Unavailable {
                    key,
                    reason: UnknownReason::MissingOriginal,
                },
            )
        };
        let value = ClientRecord {
            key,
            latest,
            retained: vec![],
            pending: Some(PendingRecord {
                key,
                original,
                working: tag(n, "new"),
                prepared: None,
                witness: None,
            }),
        };
        super::store(&mut tx, &value).await?;
        assert_eq!(super::load(&mut tx, key).await?, Some(value));
    }
    let mut conflicting = baseline.clone();
    conflicting.membership_generation = 1;
    assert!(
        lineage::insert_baseline(&mut tx, &conflicting)
            .await
            .is_err()
    );
    tx.commit().await?;
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    assert_eq!(crate::save::load(&store, key).await?, Some(response));
    for n in [20, 21] {
        assert!(
            crate::save::load(
                &store,
                RecordKey {
                    account: id(1),
                    id: RecordId::Tag(id(n))
                }
            )
            .await?
            .unwrap()
            .pending
            .is_some()
        );
    }
    store.close().await?;
    Ok(())
}
