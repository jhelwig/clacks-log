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
    let mut state = disposition.state().clone();
    let RecordState::Live(RecordValue::Service(service)) = &mut state else {
        panic!("service")
    };
    service.support.required = vec![
        SupportRequirement::ExactFact(VersionReference::from(&observation)),
        SupportRequirement::CoveredEffect(VersionReference::from(&qso)),
        SupportRequirement::CoveredEffect(VersionReference::from(&association)),
    ];
    disposition = clacks_sync::record::AcceptedRecord::new(
        disposition.key(),
        state,
        disposition.stamp.clone(),
    )?;

    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: Coverage::from(disposition.stamp.accepted_at()),
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: [&qso, &observation, &association, &disposition]
                .into_iter()
                .map(|value| RecordRequirement {
                    key: value.key(),
                    stamp: value.stamp.clone(),
                    deleted: false,
                })
                .collect(),
            boundaries: vec![],
            baselines: vec![baseline.clone()],
            units: vec![CoupledRequirement {
                id: support::id(505),
                members: [&qso, &association, &disposition]
                    .into_iter()
                    .map(VersionReference::from)
                    .collect(),
            }],
            accepted: vec![],
            support: vec![],
        },
    };

    let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming
        .page(
            &store,
            &[disposition.clone(), qso.clone(), association.clone()],
        )
        .await?;

    assert!(save::load(&store, association.key()).await?.is_none());

    store.close().await?;

    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;

    let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming.page(&store, &[observation]).await?;

    for key in [qso.key(), association.key(), disposition.key()] {
        assert!(save::load(&store, key).await?.is_some());
    }

    assert_eq!(incoming.finish(&store).await?, source);
    clacks_client_store::retention::reclaim_completed_cycle(&store).await?;

    // A different accepted binding claiming the exact same namespace identity
    // must roll back its row and progress together, rather than replacing ownership.
    let competing = carrier(
        5,
        ServicePayload::Association(ExternalAssociation {
            id: support::id(603),
            external: external.clone(),
            qso: support::id(10),
            state: AssociationState::Active,
        }),
    );
    let next_source = InstalledCheckpoint {
        coverage: Coverage::from(competing.stamp.accepted_at()),
        ..source.clone()
    };
    let incremental = WindowMode::Incremental {
        after: source.coverage.clone(),
    };
    let next = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: next_source.clone(),
            mode: incremental.clone(),
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![RecordRequirement {
                key: competing.key(),
                stamp: competing.stamp.clone(),
                deleted: false,
            }],
            boundaries: vec![],
            baselines: vec![baseline.clone()],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };

    let mut incoming = crate::incoming::Incoming::begin(&store, &next).await?;
    assert!(
        incoming
            .page(&store, std::slice::from_ref(&competing))
            .await
            .is_err()
    );

    assert!(save::load(&store, competing.key()).await?.is_none());

    assert_eq!(
        save::load(&store, association.key())
            .await?
            .unwrap()
            .latest(),
        &clacks_sync::record::Observation::Accepted(association.clone())
    );

    // A saved service draft owns its exact historical support even before acceptance.
    let mut draft = carrier(
        7,
        ServicePayload::Disposition(ProcessingDisposition {
            id: support::id(704),
            observation: support::id(501),
            decision: ProcessingDecision::Associated,
            association: Some(support::id(503)),
        }),
    );
    let mut state = draft.state().clone();
    let RecordState::Live(RecordValue::Service(service)) = &mut state else {
        panic!("service draft");
    };
    service.support.required = vec![
        SupportRequirement::ExactFact(VersionReference::from(&association)),
        SupportRequirement::CoveredEffect(VersionReference::from(&qso)),
    ];
    draft =
        clacks_sync::record::AcceptedRecord::new(draft.key(), state.clone(), draft.stamp.clone())?;
    assert!(matches!(
        save::save(&store, draft.key(), None, state).await?,
        save::SaveOutcome::Saved(_)
    ));

    let superseded = carrier(
        6,
        ServicePayload::Association(ExternalAssociation {
            id: support::id(503),
            external,
            qso: support::id(10),
            state: AssociationState::Superseded,
        }),
    );

    let mut next = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: InstalledCheckpoint {
                coverage: Coverage::from(superseded.stamp.accepted_at()),
                ..source.clone()
            },
            mode: WindowMode::Incremental {
                after: source.coverage.clone(),
            },
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            accepted: vec![superseded],
            support: vec![],
            baselines: vec![baseline],
            boundaries: vec![],
            units: vec![],
        },
    };
    let incoming = crate::incoming::Incoming::begin(&store, &next).await?;
    let checkpoint = incoming.finish(&store).await?;
    clacks_client_store::retention::reclaim_completed_cycle(&store).await?;

    next.start.mode = WindowMode::Incremental {
        after: checkpoint.coverage,
    };
    next.start.source.coverage = Coverage::from(draft.stamp.accepted_at());
    next.page.accepted = vec![draft.clone()];
    let incoming = crate::incoming::Incoming::begin(&store, &next).await?;

    assert_eq!(
        save::load(&store, draft.key()).await?.unwrap().latest(),
        &clacks_sync::record::Observation::Accepted(draft),
        "cleanup must preserve the exact body owned by the saved draft so receipt can complete without downloading it again"
    );
    incoming.finish(&store).await?;

    store.close().await?;

    Ok(())
}
