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
async fn service_effects_wait_for_the_complete_unit_and_current_ownership_is_unique()
-> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
            evidence::{ServicePayload, observation::*, processing::*},
        },
    };
    use clacks_sync::{
        lineage::Coverage,
        record::{RecordState, RecordValue},
        service::{
            ServiceRecord, ServiceSupport, SupportRequirement, VersionReference,
            operation::CoupledRequirement,
        },
        transfer::RecordRequirement,
    };
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let qso = support::accepted(1, support::contact(support::qso(support::id(10))));
    let external = ExternalRecordKey {
        destination: support::id(42),
        record: RemoteRecordId::Qrz("18446744073709551616001".parse()?),
    };
    let destination = Destination {
        id: external.destination,
        collection: RemoteCollection::Qrz { book_id: None },
        description: "Historical unknown book".into(),
    };
    let carrier = |revision, payload| {
        support::accepted(
            revision,
            RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                payload,
                support: ServiceSupport {
                    destination: Some(destination.clone()),
                    ..Default::default()
                },
            }))),
        )
    };
    let observation = carrier(
        2,
        ServicePayload::Observation(Box::new(RemoteObservation {
            id: support::id(501),
            destination: destination.id,
            identity: RemoteIdentity::Complete(external.clone()),
            facts: Default::default(),
            ordering: Default::default(),
            acquisition: AcquisitionProvenance {
                id: support::id(502),
                query: QueryIdentity {
                    destination: destination.id,
                    stream: QueryStream::Qrz,
                    selected_fields: Default::default(),
                    own_call: None,
                    since: None,
                },
                observed_at: UtcTimestamp::new(0, 0)?,
            },
        })),
    );
    let association = carrier(
        3,
        ServicePayload::Association(ExternalAssociation {
            id: support::id(503),
            external: external.clone(),
            qso: support::id(10),
            state: AssociationState::Active,
        }),
    );
    let mut disposition = carrier(
        4,
        ServicePayload::Disposition(ProcessingDisposition {
            id: support::id(504),
            observation: support::id(501),
            decision: ProcessingDecision::Associated,
            association: Some(support::id(503)),
        }),
    );
    let RecordState::Live(RecordValue::Service(service)) = &mut disposition.state else {
        panic!("service")
    };
    service.support.required = vec![
        SupportRequirement::ExactFact(VersionReference::from(&observation)),
        SupportRequirement::CoveredEffect(VersionReference::from(&qso)),
        SupportRequirement::CoveredEffect(VersionReference::from(&association)),
    ];
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: Coverage::from(disposition.stamp.accepted_at),
    };
    let manifest = WindowManifest::new(
        source.account,
        source.clone(),
        WindowMode::Full,
        [&qso, &observation, &association, &disposition]
            .into_iter()
            .map(|value| RecordRequirement {
                key: value.key,
                stamp: value.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        vec![baseline.clone()],
        vec![CoupledRequirement {
            id: support::id(505),
            members: [&qso, &association, &disposition]
                .into_iter()
                .map(VersionReference::from)
                .collect(),
        }],
    )?;
    transfer::remember(&store, &manifest).await?;
    assert!(
        transfer::install(
            &store,
            &source,
            &WindowMode::Full,
            &[disposition.clone(), qso.clone(), association.clone()]
        )
        .await?
        .accepted
        .is_empty()
    );
    assert!(save::load(&store, association.key).await?.is_none());
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(
        transfer::install(&store, &source, &WindowMode::Full, &[observation])
            .await?
            .accepted
            .len(),
        4
    );
    assert_eq!(
        transfer::finish(&store, &source, &WindowMode::Full).await?,
        source
    );
    // A different accepted binding claiming the exact same namespace identity
    // must roll back its row and progress together, rather than replacing ownership.
    let competing = carrier(
        5,
        ServicePayload::Association(ExternalAssociation {
            id: support::id(603),
            external,
            qso: support::id(10),
            state: AssociationState::Active,
        }),
    );
    let next_source = InstalledCheckpoint {
        coverage: Coverage::from(competing.stamp.accepted_at),
        ..source.clone()
    };
    let incremental = WindowMode::Incremental {
        after: source.coverage.clone(),
    };
    let next = WindowManifest::new(
        source.account,
        next_source.clone(),
        incremental.clone(),
        vec![RecordRequirement {
            key: competing.key,
            stamp: competing.stamp.clone(),
            deleted: false,
        }],
        vec![],
        vec![baseline],
        vec![],
    )?;
    transfer::remember(&store, &next).await?;
    assert!(
        transfer::install(
            &store,
            &next_source,
            &incremental,
            std::slice::from_ref(&competing)
        )
        .await
        .is_err()
    );
    assert!(save::load(&store, competing.key).await?.is_none());
    assert_eq!(
        transfer::progress(&store, &next_source, &incremental)
            .await?
            .generation,
        0
    );
    assert_eq!(
        save::load(&store, association.key).await?.unwrap().latest,
        clacks_sync::record::Observation::Accepted(association)
    );
    store.close().await?;
    Ok(())
}
