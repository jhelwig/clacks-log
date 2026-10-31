use crate::support;
use clacks_client_store::{
    save,
    store::{ClientStore, StoreSettings},
    synchronization, transfer,
};
use clacks_sync::{
    account::{AccountMetadata, InstalledCheckpoint},
    lineage::Coverage,
    record::{Observation, RecordState, RecordValue},
    service::{VersionReference, operation::CoupledRequirement},
    transfer::{
        WindowMode,
        discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
    },
};

#[tokio::test]
async fn completed_initial_receipt_survives_reopen() -> anyhow::Result<()> {
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
    let mut receipt = transfer::begin(
        &store,
        &DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
    )
    .await?;
    transfer::install_page(
        &store,
        &mut receipt,
        &DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            accepted: vec![],
            support: vec![],
            baselines: vec![baseline],
            boundaries: vec![],
            units: vec![],
        },
    )
    .await?;
    assert!(synchronization::state(&store).await?.checkpoint.is_none());
    assert_eq!(
        transfer::complete(
            &store,
            &receipt,
            &DiscoveryEnd {
                source: source.clone(),
                pages: 1
            }
        )
        .await?,
        source
    );
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(
        synchronization::state(&store).await?.checkpoint,
        Some(source)
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn partial_unit_survives_a_new_discovery_attempt() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let qso = support::accepted(1, support::contact(support::qso(support::id(10))));
    let tag = support::accepted(
        2,
        RecordState::Live(RecordValue::Tag(Box::new(
            clacks_model::tag::LogbookTag::new(support::id(20), "Tag".into()),
        ))),
    );
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: Coverage::from(tag.stamp.accepted_at()),
    };
    let start = DiscoveryStart {
        source: source.clone(),
        mode: WindowMode::Full,
    };
    let unit = CoupledRequirement {
        id: support::id(90),
        members: [&qso, &tag]
            .into_iter()
            .map(VersionReference::from)
            .collect(),
    };
    let mut receipt = transfer::begin(&store, &start).await?;
    let page = DiscoveryPage {
        sequence: 0,
        requirements: vec![],
        accepted: vec![qso.clone()],
        support: vec![],
        baselines: vec![baseline.clone()],
        boundaries: vec![],
        units: vec![unit.clone()],
    };
    transfer::install_page(&store, &mut receipt, &page).await?;
    assert!(save::load(&store, qso.key()).await?.is_none());
    assert!(
        transfer::complete(
            &store,
            &receipt,
            &DiscoveryEnd {
                source: source.clone(),
                pages: 1
            }
        )
        .await
        .is_err()
    );
    drop(receipt);
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert!(synchronization::state(&store).await?.checkpoint.is_none());
    let mut repair =
        clacks_client_store::discovery::Source::repair(&store, &Default::default()).await?;
    let retained = repair.page(&store, 10).await?.unwrap();
    assert!(retained.support.contains(&qso));
    assert_eq!(retained.units, vec![unit.clone()]);
    let mut receipt = transfer::begin(&store, &start).await?;
    transfer::install_page(
        &store,
        &mut receipt,
        &DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            accepted: vec![tag.clone()],
            support: vec![],
            baselines: vec![baseline],
            boundaries: vec![],
            units: vec![unit],
        },
    )
    .await?;
    assert!(save::load(&store, qso.key()).await?.is_some());
    assert!(save::load(&store, tag.key()).await?.is_some());
    assert_eq!(
        transfer::complete(
            &store,
            &receipt,
            &DiscoveryEnd {
                source: source.clone(),
                pages: 1
            }
        )
        .await?,
        source
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn interrupted_discovery_keeps_data_and_pending_work_without_coverage() -> anyhow::Result<()>
{
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let value = support::accepted(2, support::contact(support::qso(support::id(10))));
    let older = support::accepted(1, support::contact(support::qso(support::id(10))));
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: value.stamp.accepted_at().into(),
    };
    let start = DiscoveryStart {
        source: source.clone(),
        mode: WindowMode::Full,
    };
    let local_key = support::qso_key(support::id(11));
    save::save(
        &store,
        local_key,
        None,
        support::contact(support::qso(support::id(11))),
    )
    .await?;
    let pending = save::load(&store, local_key).await?.unwrap();
    let page = DiscoveryPage {
        sequence: 0,
        requirements: vec![],
        accepted: vec![value.clone()],
        support: vec![older],
        baselines: vec![baseline],
        boundaries: vec![],
        units: vec![],
    };
    let mut receipt = transfer::begin(&store, &start).await?;
    transfer::install_page(&store, &mut receipt, &page).await?;
    assert!(
        transfer::complete(
            &store,
            &receipt,
            &DiscoveryEnd {
                source: source.clone(),
                pages: 0
            }
        )
        .await
        .is_err()
    );
    assert!(synchronization::state(&store).await?.checkpoint.is_none());
    drop(receipt);
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(save::load(&store, local_key).await?, Some(pending));
    assert_eq!(
        save::load(&store, value.key()).await?.unwrap().latest(),
        &Observation::Accepted(value.clone())
    );
    let mut receipt = transfer::begin(&store, &start).await?;
    transfer::install_page(&store, &mut receipt, &page).await?;
    assert_eq!(
        transfer::complete(
            &store,
            &receipt,
            &DiscoveryEnd {
                source: source.clone(),
                pages: 1
            }
        )
        .await?,
        source
    );
    assert_eq!(
        save::load(&store, value.key()).await?.unwrap().latest(),
        &Observation::Accepted(value)
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn recovery_offers_accepted_tail_and_retained_only_values_without_claiming_coverage()
-> anyhow::Result<()> {
    use clacks_client_store::{discovery::Source, synchronization};
    use clacks_sync::{
        client::{OriginalBasis, ReviewInput, ReviewReason},
        lineage::Coverage,
        record::Observation,
    };
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: Coverage::from_entries([(support::id(2), 100)])?,
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            boundaries: vec![],
            baselines: vec![baseline],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };
    crate::incoming::Incoming::begin(&store, &manifest)
        .await?
        .finish(&store)
        .await?;
    let accepted = support::accepted(101, support::contact(support::qso(support::id(10))));
    synchronization::receive(&store, &accepted).await?;
    assert_eq!(
        synchronization::state(&store).await?.checkpoint,
        Some(source.clone())
    );
    let mut repair = Source::repair(&store, &source.coverage).await?;
    assert!(repair.start.is_none());
    assert!(
        repair
            .page(&store, 10)
            .await?
            .unwrap()
            .accepted
            .contains(&accepted)
    );
    let current = save::load(&store, accepted.key()).await?.unwrap();
    let mut working = support::qso(support::id(10)).into_fields();
    working.comment = Some("Unpublished correction".into());
    save::save(
        &store,
        accepted.key(),
        Some(&current),
        support::contact(working.try_into()?),
    )
    .await?;
    synchronization::receive_review(
        &store,
        &ReviewInput {
            original: OriginalBasis::Unavailable {
                key: accepted.key(),
                reason: clacks_sync::error::UnknownReason::MissingOriginal,
            },
            local: accepted.state().clone(),
            shared: Observation::Absent {
                key: accepted.key(),
                checkpoint: source.clone(),
            },
            reason: ReviewReason::MissingOriginal,
            retained: vec![accepted.clone()],
        },
    )
    .await?;
    let mut repair = Source::repair(&store, &source.coverage).await?;
    let page = repair.page(&store, 10).await?.unwrap();
    assert!(page.accepted.is_empty());
    assert_eq!(page.support, vec![accepted.clone()]);
    assert!(repair.page(&store, 10).await?.is_none());
    assert!(repair.end()?.is_none());
    let mut full = Source::start(&store).await?;
    assert!(
        full.page(&store, 10)
            .await?
            .unwrap()
            .support
            .contains(&accepted)
    );
    assert_eq!(
        synchronization::state(&store).await?.checkpoint,
        Some(source)
    );
    store.close().await?;
    Ok(())
}
