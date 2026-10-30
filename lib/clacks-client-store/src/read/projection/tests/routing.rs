use super::*;
use clacks_model::service::{
    destination::{ExternalRecordKey, RemoteRecordId},
    evidence::processing::{AssociationState, ExternalAssociation},
};

#[tokio::test]
async fn qrz_confirmation_does_not_cross_an_explicit_book_association() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("history.sqlite"),
        support::id(1),
        &account_definitions()?,
        StoreSettings::default(),
    )
    .await?;
    let qso = support::qso(support::id(10));
    save::save(
        &store,
        support::qso_key(qso.id()),
        None,
        support::contact(qso.clone()),
    )
    .await?;
    let mut state = imported(&qso, 40, Service::Qrz, ImportedClaim::Confirmed)?;
    let RecordState::Live(RecordValue::Service(s)) = &mut state else {
        panic!("service")
    };
    let ServicePayload::Imported(a) = &mut s.payload else {
        panic!("assertion")
    };
    a.destination = Some(support::id(100));
    let key = support::key(&RecordValue::Service(s.clone()));
    save::save(&store, key, None, state).await?;
    let association = RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Association(ExternalAssociation {
            id: support::id(50),
            external: ExternalRecordKey {
                destination: support::id(200),
                record: RemoteRecordId::Qrz("7".parse()?),
            },
            qso: qso.id(),
            state: AssociationState::Active,
        }),
        support: Default::default(),
    }));
    save::save(
        &store,
        support::key(&association),
        None,
        RecordState::Live(association),
    )
    .await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id()],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    assert!(!facts.first().unwrap().facts.qrz.confirmed);
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn a_later_acquired_unconfirmed_report_supersedes_an_imported_positive() -> anyhow::Result<()>
{
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{Destination, RemoteCollection},
            evidence::{observation::*, processing::*},
        },
    };
    use clacks_sync::service::{ServiceSupport, SupportRequirement, VersionReference};
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("history.sqlite"),
        support::id(1),
        &account_definitions()?,
        StoreSettings::default(),
    )
    .await?;
    let qso = support::qso(support::id(10));
    let original = support::accepted(1, support::contact(qso.clone()));
    let imported = support::accepted(
        2,
        imported(&qso, 40, Service::Qrz, ImportedClaim::Confirmed)?,
    );
    let RecordState::Live(RecordValue::Service(s)) = &imported.state else {
        panic!("service")
    };
    let ServicePayload::Imported(a) = &s.payload else {
        panic!("import")
    };
    let baseline = a.baseline.confirmation.clone();
    let destination = Destination {
        id: support::id(100),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse()?),
        },
        description: "Book".into(),
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
    let external = ExternalRecordKey {
        destination: destination.id,
        record: RemoteRecordId::Qrz("7".parse()?),
    };
    let mut observation = carrier(
        4,
        ServicePayload::Observation(Box::new(RemoteObservation {
            id: support::id(50),
            destination: destination.id,
            identity: RemoteIdentity::Complete(external.clone()),
            facts: RemoteFacts {
                contact: ContactReport {
                    own_call: Reported::Value(qso.own_station().callsign.clone()),
                    worked_call: Reported::Value(qso.contacted_station().callsign.clone()),
                    start: Reported::Value(ObservedStart::Second(qso.start())),
                    frequency: Reported::Value(qso.primary().frequency().unwrap()),
                    receive_frequency: Reported::Value(qso.primary().frequency().unwrap()),
                    mode: Reported::Value(baseline.mode.unwrap()),
                    confirmation: Reported::Value(ConfirmationStatus::Unconfirmed),
                    ..Default::default()
                },
                ..Default::default()
            },
            ordering: Default::default(),
            acquisition: AcquisitionProvenance {
                id: support::id(51),
                query: QueryIdentity {
                    destination: destination.id,
                    stream: QueryStream::Qrz,
                    selected_fields: Default::default(),
                    own_call: None,
                    since: None,
                },
                observed_at: UtcTimestamp::new(1000, 0)?,
            },
        })),
    );
    let RecordState::Live(RecordValue::Service(s)) = &mut observation.state else {
        panic!("service")
    };
    s.support.acquisition = Some(support::accepted(3, support::contact(qso.clone())).stamp);
    let association = carrier(
        5,
        ServicePayload::Association(ExternalAssociation {
            id: support::id(52),
            external,
            qso: qso.id(),
            state: AssociationState::Active,
        }),
    );
    let mut disposition = carrier(
        6,
        ServicePayload::Disposition(ProcessingDisposition {
            id: support::id(53),
            observation: support::id(50),
            decision: ProcessingDecision::Associated,
            association: Some(support::id(52)),
        }),
    );
    let RecordState::Live(RecordValue::Service(s)) = &mut disposition.state else {
        panic!("service")
    };
    s.support.required = vec![
        SupportRequirement::ExactFact(VersionReference::from(&observation)),
        SupportRequirement::CoveredEffect(VersionReference::from(&original)),
        SupportRequirement::CoveredEffect(VersionReference::from(&association)),
    ];
    super::install(
        &store,
        &[original, imported, observation, association, disposition],
    )
    .await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id()],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    assert!(!facts.first().unwrap().facts.qrz.confirmed);
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}
