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
async fn incremental_acquisition_loads_dependencies_added_since_the_local_version()
-> anyhow::Result<()> {
    incremental_service_dependencies(true).await
}

#[tokio::test]
async fn incremental_service_loads_its_stored_destination_without_embedded_support()
-> anyhow::Result<()> {
    incremental_service_dependencies(false).await
}

async fn incremental_service_dependencies(update_acquisition: bool) -> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
            evidence::{
                ServicePayload,
                download::{AcquisitionPhase, AcquisitionState, DownloadControl},
                observation::{
                    AcquisitionProvenance, QueryIdentity, QueryStream, RemoteIdentity,
                    RemoteObservation,
                },
            },
        },
    };
    use clacks_sync::{
        lineage::Coverage,
        record::{AcceptedRecord, RecordState, RecordValue},
        service::{ServiceRecord, ServiceSupport},
        transfer::RecordRequirement,
    };
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse()?),
        },
        description: "Test book".into(),
    };
    let carrier = |revision, payload, embedded: bool| {
        support::accepted(
            revision,
            RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                payload,
                support: ServiceSupport {
                    destination: embedded.then(|| destination.clone()),
                    ..Default::default()
                },
            }))),
        )
    };
    let query = QueryIdentity {
        destination: destination.id,
        stream: QueryStream::Qrz,
        selected_fields: Default::default(),
        own_call: None,
        since: None,
    };
    let observation = carrier(
        1,
        ServicePayload::Observation(Box::new(RemoteObservation {
            id: support::id(60),
            destination: destination.id,
            identity: RemoteIdentity::Complete(ExternalRecordKey {
                destination: destination.id,
                record: RemoteRecordId::Qrz("8".parse()?),
            }),
            facts: Default::default(),
            ordering: Default::default(),
            acquisition: AcquisitionProvenance {
                id: support::id(49),
                query: query.clone(),
                observed_at: UtcTimestamp::new(0, 0)?,
            },
        })),
        true,
    );
    let mut acquisition = AcquisitionState {
        id: support::id(50),
        query,
        phase: AcquisitionPhase::Active,
        started_at: UtcTimestamp::new(10, 0)?,
        retained_inputs: Default::default(),
    };
    let rows = vec![
        observation,
        carrier(2, ServicePayload::Acquisition(acquisition.clone()), true),
        carrier(3, ServicePayload::Destination(destination.clone()), false),
    ];
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: Coverage::from(rows.last().expect("seed rows").stamp.accepted_at()),
    };
    let requirements = |rows: &[AcceptedRecord]| {
        rows.iter()
            .map(|value| RecordRequirement {
                key: value.key(),
                stamp: value.stamp.clone(),
                deleted: false,
            })
            .collect()
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: requirements(&rows),
            boundaries: vec![],
            baselines: vec![baseline.clone()],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };
    let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming.page(&store, &rows).await?;
    for row in &rows {
        assert!(save::load(&store, row.key()).await?.is_some());
    }
    incoming.finish(&store).await?;
    let next = if update_acquisition {
        acquisition.retained_inputs.insert(support::id(60));
        carrier(4, ServicePayload::Acquisition(acquisition), true)
    } else {
        carrier(
            4,
            ServicePayload::Download(DownloadControl::new(destination.id)?),
            false,
        )
    };
    let mode = WindowMode::Incremental {
        after: source.coverage.clone(),
    };
    let next_source = InstalledCheckpoint {
        coverage: Coverage::from(next.stamp.accepted_at()),
        ..source
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: next_source.clone(),
            mode: mode.clone(),
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: requirements(std::slice::from_ref(&next)),
            boundaries: vec![],
            baselines: vec![baseline],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };
    let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming.page(&store, std::slice::from_ref(&next)).await?;
    incoming.page(&store, &[]).await?;
    assert_eq!(
        save::load(&store, next.key())
            .await?
            .expect("installed row")
            .latest(),
        &clacks_sync::record::Observation::Accepted(next)
    );
    assert_eq!(incoming.finish(&store).await?, next_source);
    store.close().await?;
    Ok(())
}
