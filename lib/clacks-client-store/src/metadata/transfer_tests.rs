use super::*;
use crate::metadata::client;
use crate::store::{ClientStore, StoreSettings};
use clacks_model::tag::LogbookTag;
use clacks_sync::{
    acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange, plan_server_acceptance},
    account::{AccountMetadata, EvidenceView},
    client::VersionCondition,
    membership::{ClientMembership, MembershipSnapshot, revoke},
    record::{AcceptedRecord, Observation, RecordId, RecordState, RecordValue},
    transfer::plan_batch,
};

fn id<T: std::str::FromStr>(n: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(n).to_string().parse().unwrap()
}

#[tokio::test]
async fn immutable_windows_and_staged_units_survive_restart_without_claiming_receipt()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let (mut account, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let origin = account.head;
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };
    let mut membership = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![
            ClientMembership {
                actor: id(9),
                created_at: origin,
                closed_at: None,
                acknowledged: None,
            },
            ClientMembership {
                actor: id(8),
                created_at: origin,
                closed_at: None,
                acknowledged: None,
            },
        ],
    };
    let mut accepted = Vec::new();
    for n in [10, 11] {
        let key = RecordKey {
            account: id(1),
            id: RecordId::Tag(id(n)),
        };
        let current = Observation::Absent {
            key,
            checkpoint: InstalledCheckpoint {
                account: id(1),
                baseline: id(3),
                coverage: account.installed.clone(),
            },
        };
        let change = ServerChange {
            key,
            state: RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
                id(n),
                format!("Tag {n}"),
            )))),
            expected: VersionCondition::from_observation(&current)?,
            dependencies: vec![],
        };
        let input = AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &membership,
            current: &current,
            references: &[],
        };
        let AcceptanceDecision::Commit(commit) =
            plan_server_acceptance(&change, &input, &evidence)?
        else {
            panic!("source creation expected")
        };
        account = commit.next_account;
        accepted.push(commit.accepted);
    }
    let acknowledged = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: account.installed.clone(),
    };
    membership.clients.first_mut().unwrap().acknowledged = Some(acknowledged);
    let closed = revoke(id(8), &account, &membership)?;
    let account = closed.next_account;
    let membership = closed.next;
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: account.installed.clone(),
    };
    let manifest = WindowManifest::new(
        id(1),
        source.clone(),
        WindowMode::Full,
        accepted
            .iter()
            .map(|record| RecordRequirement {
                key: record.key,
                stamp: record.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        vec![baseline.clone()],
        vec![CoupledRequirement {
            id: id(900),
            members: accepted.iter().map(VersionReference::from).collect(),
        }],
    )?;
    let mut tx = store.write().await?;
    let window = remember(&mut tx, &manifest).await?;
    assert_eq!(remember(&mut tx, &manifest).await?, window);
    super::super::account::store(&mut tx, &account, &membership).await?;
    assert_eq!(
        super::super::account::load(&mut tx, id(1)).await?,
        Some((account.clone(), membership.clone()))
    );
    super::super::lineage::set_lineage(&mut tx, &baseline.lineage).await?;
    let initial = progress(&mut tx, id(1), window).await?;
    let first: &AcceptedRecord = accepted.first().unwrap();
    let partial = plan_batch(
        &manifest,
        &initial,
        std::slice::from_ref(first),
        &[],
        &evidence,
    )?;
    assert!(partial.accepted.is_empty());
    assert_eq!(partial.staged.len(), 1);
    store_batch(&mut tx, id(1), window, &partial).await?;
    assert!(
        super::super::lineage::current_checkpoint(&mut tx, id(1))
            .await?
            .is_none()
    );
    tx.commit().await?;
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let mut tx = store.write().await?;
    let staged = progress(&mut tx, id(1), window).await?;
    assert!(staged.installed.is_empty());
    assert_eq!(staged.staged, vec![first.clone()]);
    assert_eq!(
        super::super::account::load(&mut tx, id(1)).await?,
        Some((account, membership))
    );
    assert!(
        super::super::lineage::current_checkpoint(&mut tx, id(1))
            .await?
            .is_none()
    );
    let next = accepted.last().unwrap();
    let complete = plan_batch(
        &manifest,
        &staged,
        std::slice::from_ref(next),
        &[],
        &evidence,
    )?;
    assert_eq!(complete.accepted.len(), 2);
    for update in &complete.client_updates {
        client::store(&mut tx, &update.record).await?;
    }
    store_batch(&mut tx, id(1), window, &complete).await?;
    assert!(
        store_batch(&mut tx, id(1), window, &complete)
            .await
            .is_err()
    );
    let installed = progress(&mut tx, id(1), window).await?;
    assert_eq!(installed.installed.len(), 2);
    assert_eq!(installed.completed_units, [id(900)].into());
    assert!(installed.staged.is_empty());
    assert!(
        super::super::lineage::current_checkpoint(&mut tx, id(1))
            .await?
            .is_none()
    );
    let mut changed = manifest.clone();
    changed.records.first_mut().unwrap().deleted = true;
    assert!(remember(&mut tx, &changed).await.is_err());
    assert_eq!(load_manifest(&mut tx, id(1), window).await?, manifest);
    tx.commit().await?;
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let mut tx = store.pool.begin().await?;
    assert_eq!(progress(&mut tx, id(1), window).await?, installed);
    tx.commit().await?;
    store.close().await?;
    Ok(())
}
