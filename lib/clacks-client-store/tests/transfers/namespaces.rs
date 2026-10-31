use crate::support;
use clacks_client_store::{
    save,
    store::{ClientStore, StoreSettings},
};
use clacks_sync::{
    account::{AccountMetadata, InstalledCheckpoint},
    transfer::WindowMode,
};

#[tokio::test]
async fn accepted_namespaces_must_be_distinct_even_without_an_external_binding()
-> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::ServicePayload,
    };
    use clacks_sync::{
        lineage::Coverage,
        record::{RecordState, RecordValue},
        service::{ServiceRecord, ServiceSupport},
        transfer::RecordRequirement,
    };
    for second in [None, Some("7".parse()?)] {
        let directory = tempfile::tempdir()?;
        let store = ClientStore::create(
            &directory.path().join("workspace.sqlite"),
            support::id(1),
            StoreSettings::default(),
        )
        .await?;
        let (_, baseline) =
            AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
        let rows: Vec<_> = [(1, 42, Some("7".parse()?)), (2, 43, second)]
            .into_iter()
            .map(|(revision, id, book_id)| {
                support::accepted(
                    revision,
                    RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                        payload: ServicePayload::Destination(Destination {
                            id: support::id(id),
                            collection: RemoteCollection::Qrz { book_id },
                            description: "Namespace".into(),
                        }),
                        support: ServiceSupport::default(),
                    }))),
                )
            })
            .collect();
        let source = InstalledCheckpoint {
            account: support::id(1),
            baseline: baseline.id,
            coverage: Coverage::from(rows.last().unwrap().stamp.accepted_at()),
        };
        let manifest = crate::incoming::Source {
            start: clacks_sync::transfer::discovery::DiscoveryStart {
                source: source.clone(),
                mode: WindowMode::Full,
            },
            page: clacks_sync::transfer::discovery::DiscoveryPage {
                sequence: 0,
                requirements: rows
                    .iter()
                    .map(|value| RecordRequirement {
                        key: value.key(),
                        stamp: value.stamp.clone(),
                        deleted: false,
                    })
                    .collect(),
                boundaries: vec![],
                baselines: vec![baseline],
                units: vec![],
                accepted: vec![],
                support: vec![],
            },
        };
        let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
        assert!(incoming.page(&store, &rows).await.is_err());
        for row in rows {
            assert!(save::load(&store, row.key()).await?.is_none());
        }
        store.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn known_snapshot_refines_a_historical_unknown_book_without_rewriting_it()
-> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
        evidence::{ServicePayload, processing::DeletionSuppression},
    };
    use clacks_sync::{
        lineage::Coverage,
        record::{RecordState, RecordValue},
        service::{ServiceRecord, ServiceSupport},
        transfer::RecordRequirement,
    };
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let old = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: None },
        description: "Historical".into(),
    };
    let known = Destination {
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse()?),
        },
        ..old.clone()
    };
    let rows = [
        support::accepted(
            1,
            RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                payload: ServicePayload::Destination(old),
                support: ServiceSupport::default(),
            }))),
        ),
        support::accepted(
            2,
            RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                payload: ServicePayload::Suppression(DeletionSuppression {
                    id: support::id(400),
                    qso: support::id(10),
                    external: ExternalRecordKey {
                        destination: known.id,
                        record: RemoteRecordId::Qrz("7".parse()?),
                    },
                }),
                support: ServiceSupport {
                    destination: Some(known),
                    ..Default::default()
                },
            }))),
        ),
    ];
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: Coverage::from(rows.last().unwrap().stamp.accepted_at()),
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: rows
                .iter()
                .map(|value| RecordRequirement {
                    key: value.key(),
                    stamp: value.stamp.clone(),
                    deleted: false,
                })
                .collect(),
            boundaries: vec![],
            baselines: vec![baseline],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };
    let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming.page(&store, &rows).await?;
    for row in rows {
        assert_eq!(
            save::load(&store, row.key()).await?.unwrap().latest(),
            &clacks_sync::record::Observation::Accepted(row)
        );
    }
    store.close().await?;
    Ok(())
}
