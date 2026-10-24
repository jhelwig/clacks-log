use crate::support;
use clacks_client_store::{
    save,
    store::{ClientStore, StoreSettings},
    transfer,
};
use clacks_sync::{
    account::{AccountMetadata, InstalledCheckpoint},
    transfer::{WindowManifest, WindowMode},
};

#[tokio::test]
async fn initial_full_window_reloads_only_a_completed_receipt() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    let (account, baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let source = InstalledCheckpoint {
        account: account.account,
        baseline: baseline.id,
        coverage: account.installed,
    };
    let manifest = WindowManifest::new(
        source.account,
        source.clone(),
        WindowMode::Full,
        vec![],
        vec![],
        vec![baseline],
        vec![],
    )?;
    assert!(
        transfer::finish(&store, &source, &WindowMode::Full)
            .await
            .is_err()
    );
    transfer::remember(&store, &manifest).await?;
    transfer::remember(&store, &manifest).await?;
    assert_eq!(
        transfer::finish(&store, &source, &WindowMode::Full).await?,
        source
    );
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(
        transfer::finish(&store, &source, &WindowMode::Full).await?,
        source
    );
    assert!(
        save::load(&store, support::qso_key(support::id(10)))
            .await?
            .is_none()
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn partial_unit_is_durable_and_cannot_be_acknowledged_or_replaced() -> anyhow::Result<()> {
    use clacks_model::tag::LogbookTag;
    use clacks_sync::{
        lineage::Coverage,
        record::{RecordState, RecordValue},
        service::{VersionReference, operation::CoupledRequirement},
        transfer::RecordRequirement,
    };
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let qso = support::accepted(1, support::contact(support::qso(support::id(10))));
    let tag = support::accepted(
        2,
        RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
            support::id(20),
            "Tag".into(),
        )))),
    );
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: Coverage::from(tag.stamp.accepted_at),
    };
    let manifest = WindowManifest::new(
        source.account,
        source.clone(),
        WindowMode::Full,
        [&qso, &tag]
            .into_iter()
            .map(|value| RecordRequirement {
                key: value.key,
                stamp: value.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        vec![baseline],
        vec![CoupledRequirement {
            id: support::id(90),
            members: [&qso, &tag]
                .into_iter()
                .map(VersionReference::from)
                .collect(),
        }],
    )?;
    transfer::remember(&store, &manifest).await?;
    let mut changed = manifest.clone();
    changed.units.clear();
    assert!(transfer::remember(&store, &changed).await.is_err());
    let batch = transfer::install(
        &store,
        &source,
        &WindowMode::Full,
        std::slice::from_ref(&qso),
    )
    .await?;
    assert!(batch.accepted.is_empty());
    assert!(save::load(&store, qso.key).await?.is_none());
    assert!(
        transfer::finish(&store, &source, &WindowMode::Full)
            .await
            .is_err()
    );
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    let progress = transfer::progress(&store, &source, &WindowMode::Full).await?;
    assert_eq!(progress.staged, vec![qso.clone()]);
    let batch = transfer::install(
        &store,
        &source,
        &WindowMode::Full,
        std::slice::from_ref(&tag),
    )
    .await?;
    assert_eq!(batch.accepted.len(), 2);
    assert!(batch.staged.is_empty());
    assert!(save::load(&store, qso.key).await?.is_some());
    assert_eq!(
        transfer::finish(&store, &source, &WindowMode::Full).await?,
        source
    );
    store.close().await?;
    Ok(())
}
