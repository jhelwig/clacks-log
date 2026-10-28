use super::projection::{self, ProjectionNeeds};
use crate::test_support::service::imported;
use crate::{
    read, save,
    store::{ClientStore, StoreSettings},
    test_support as support,
};
use clacks_model::{
    defaults::account_definitions,
    scalar::Milliwatts,
    service::{
        destination::Service,
        evidence::{ImportedClaim, ServicePayload},
    },
};
use clacks_sync::{
    record::{RecordState, RecordValue},
    service::ServiceRecord,
};

#[path = "tests/routing.rs"]
mod routing;
#[path = "tests/submissions.rs"]
mod submissions;

#[tokio::test]
async fn retained_evidence_projects_current_service_status() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("history.sqlite"),
        support::id(1),
        &account_definitions()?,
        StoreSettings::default(),
    )
    .await?;
    let mut qso = support::qso(support::id(10));
    let row = save::save(
        &store,
        support::qso_key(qso.id),
        None,
        support::contact(qso.clone()),
    )
    .await?;
    for (n, service, claim) in [
        (40, Service::Qrz, ImportedClaim::Uploaded),
        (41, Service::Qrz, ImportedClaim::Confirmed),
        (42, Service::Lotw, ImportedClaim::Uploaded),
        (43, Service::Lotw, ImportedClaim::Confirmed),
    ] {
        let value = imported(&qso, n, service, claim)?;
        let RecordState::Live(v) = &value else {
            panic!("live import")
        };
        save::save(&store, support::key(v), None, value.clone()).await?;
    }
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    let fact = facts.first().unwrap();
    assert!(fact.facts.qrz.uploaded && fact.facts.qrz.confirmed);
    assert!(fact.facts.lotw.uploaded && fact.facts.lotw.confirmed);
    snapshot.close().await?;
    qso.own_context.transmit_power = Some(Milliwatts::new(100_000)?);
    let save::SaveOutcome::Saved(row) = row else {
        panic!("save")
    };
    save::save(&store, row.key, Some(&row), support::contact(qso.clone())).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    let fact = facts.first().unwrap();
    assert!(!fact.facts.qrz.uploaded);
    assert!(fact.facts.qrz.confirmed);
    assert!(fact.facts.lotw.uploaded && fact.facts.lotw.confirmed);
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}

/// Install ordinary accepted records through the production full-transfer path.
async fn install(
    store: &ClientStore,
    rows: &[clacks_sync::record::AcceptedRecord],
) -> anyhow::Result<()> {
    use clacks_sync::{
        account::{AccountMetadata, InstalledCheckpoint},
        lineage::Coverage,
        transfer::{RecordRequirement, WindowManifest, WindowMode},
    };
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: baseline.id,
        coverage: Coverage::from(rows.last().unwrap().stamp.accepted_at),
    };
    let manifest = WindowManifest::new(
        source.account,
        source.clone(),
        WindowMode::Full,
        rows.iter()
            .map(|r| RecordRequirement {
                key: r.key,
                stamp: r.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        vec![baseline],
        vec![],
    )?;
    crate::transfer::remember(store, &manifest).await?;
    crate::transfer::install(store, &source, &WindowMode::Full, rows).await?;
    crate::transfer::finish(store, &source, &WindowMode::Full).await?;
    Ok(())
}

#[tokio::test]
async fn effective_facts_preserve_source_and_station_side() -> anyhow::Result<()> {
    use clacks_model::{
        contact::{BandFrequency, OperatorInformation},
        effective::InformationSource,
        location::RecordedLocation,
        position::{Locator, Position},
        recognition::band::Band,
        reference::ReferenceAssociation,
        scalar::UtcTimestamp,
        service::{
            destination::{
                Destination, ExternalRecordKey, LotwRecordKey, RemoteCollection, RemoteRecordId,
            },
            evidence::{observation::*, processing::*},
        },
    };
    use clacks_sync::service::{ServiceSupport, SupportRequirement, VersionReference};
    let directory = tempfile::tempdir()?;
    let seed = account_definitions()?;
    let pota = seed.groups.iter().find(|g| g.name == "POTA").unwrap().id;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("history.sqlite"),
        support::id(1),
        &seed,
        StoreSettings::default(),
    )
    .await?;
    let mut qso = support::qso(support::id(10));
    qso.primary = BandFrequency::from_recorded_pair(
        qso.primary.frequency(),
        Some(Band::from_recorded_label("20M")?),
    )?;
    let original = support::accepted(1, support::contact(qso.clone()));
    let mut rows = vec![original.clone()];
    for (n, service, name, grid) in [
        (100, Service::Lotw, "LoTW name", "CN85ab"),
        (200, Service::Qrz, "QRZ name", "CN84"),
    ] {
        let destination = Destination {
            id: support::id(n),
            collection: if service == Service::Lotw {
                RemoteCollection::Lotw {
                    account: "personal".into(),
                }
            } else {
                RemoteCollection::Qrz {
                    book_id: Some("7".parse()?),
                }
            },
            description: format!("{service:?}"),
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
            record: if service == Service::Lotw {
                RemoteRecordId::Lotw(LotwRecordKey {
                    own_call: qso.own_station.callsign.clone(),
                    worked_call: qso.contacted_station.callsign.clone(),
                    start: qso.start,
                    band: "20M".into(),
                    receive_band: "20M".into(),
                    mode: "CW".into(),
                })
            } else {
                RemoteRecordId::Qrz("12".parse()?)
            },
        };
        let state = imported(&qso, n + 10, service, ImportedClaim::Confirmed)?;
        let RecordState::Live(RecordValue::Service(value)) = state else {
            panic!("imported fixture")
        };
        let ServicePayload::Imported(value) = value.payload else {
            panic!("baseline")
        };
        let basis = value.baseline.confirmation;
        let observation = carrier(
            rows.len() as u64 + 1,
            ServicePayload::Observation(Box::new(RemoteObservation {
                id: support::id(n + 1),
                destination: destination.id,
                identity: RemoteIdentity::Complete(external.clone()),
                facts: RemoteFacts {
                    contact: ContactReport {
                        own_call: Reported::Value(qso.own_station.callsign.clone()),
                        worked_call: Reported::Value(qso.contacted_station.callsign.clone()),
                        start: Reported::Value(ObservedStart::Second(qso.start)),
                        band: Reported::Value("20M".into()),
                        receive_band: Reported::Value("20M".into()),
                        mode: Reported::Value(basis.mode.unwrap()),
                        confirmation: Reported::Value(ConfirmationStatus::Confirmed),
                        ..Default::default()
                    },
                    own_station: StationReport {
                        name: Reported::Value("Our echoed name".into()),
                        ..Default::default()
                    },
                    counterpart: Some(StationReport {
                        name: Reported::Value(name.into()),
                        position: Reported::Value(Position::GridArea(Locator::parse(grid)?)),
                        ..Default::default()
                    }),
                },
                ordering: ProviderOrdering {
                    qso: Some(qso.start),
                    confirmation: Some(qso.start),
                },
                acquisition: AcquisitionProvenance {
                    id: support::id(n + 2),
                    query: QueryIdentity {
                        destination: destination.id,
                        stream: if service == Service::Lotw {
                            QueryStream::LotwConfirmation
                        } else {
                            QueryStream::Qrz
                        },
                        selected_fields: Default::default(),
                        own_call: None,
                        since: None,
                    },
                    observed_at: UtcTimestamp::new(0, 0)?,
                },
            })),
        );
        let association = carrier(
            rows.len() as u64 + 2,
            ServicePayload::Association(ExternalAssociation {
                id: support::id(n + 3),
                external,
                qso: qso.id,
                state: AssociationState::Active,
            }),
        );
        let mut disposition = carrier(
            rows.len() as u64 + 3,
            ServicePayload::Disposition(ProcessingDisposition {
                id: support::id(n + 4),
                observation: support::id(n + 1),
                decision: ProcessingDecision::Associated,
                association: Some(support::id(n + 3)),
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
        rows.extend([observation, association, disposition]);
    }
    install(&store, &rows).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    let fact = &facts.first().unwrap().facts;
    assert_eq!(fact.name.as_ref().unwrap().value, "LoTW name");
    assert_eq!(fact.name.as_ref().unwrap().source, InformationSource::Lotw);
    assert_eq!(
        fact.contact_location
            .as_ref()
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .position,
        Some(Position::GridArea(Locator::parse("CN85ab")?))
    );
    assert!(fact.own_location.is_none());
    let old = snapshot.record(support::qso_key(qso.id)).await?.unwrap();
    snapshot.close().await?;
    qso.contacted_station.location = Some(RecordedLocation {
        dxcc: Some("291".into()),
        ..Default::default()
    });
    qso.contacted_operator = Some(OperatorInformation {
        callsign: None,
        recorded_name: Some("Recorded name".into()),
    });
    let save::SaveOutcome::Saved(old) =
        save::save(&store, old.key, Some(&old), support::contact(qso.clone())).await?
    else {
        panic!("save")
    };
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    let fact = &facts.first().unwrap().facts;
    assert_eq!(fact.name.as_ref().unwrap().value, "Recorded name");
    assert!(
        fact.contact_location
            .as_ref()
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .position
            .is_none()
    );
    assert_eq!(
        fact.contact_location.as_ref().unwrap().source,
        InformationSource::Recorded
    );
    snapshot.close().await?;
    qso.contacted_station.location = None;
    qso.contacted_operator = None;
    qso.contacted_station
        .references
        .insert(ReferenceAssociation {
            group_id: pota,
            identifier: Some("US-0001".into()),
        });
    save::save(&store, old.key, Some(&old), support::contact(qso.clone())).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let facts = projection::load(
        &mut snapshot.transaction,
        snapshot.account,
        &[qso.id],
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;
    let fact = &facts.first().unwrap().facts;
    assert!(fact.contact_location.as_ref().unwrap().value.is_none());
    assert_eq!(fact.name.as_ref().unwrap().value, "LoTW name");
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}
