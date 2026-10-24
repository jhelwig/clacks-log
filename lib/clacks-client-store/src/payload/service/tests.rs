use super::*;
use crate::store::{ClientStore, StoreSettings};
use clacks_model::{
    ids::DestinationId,
    location::{CqZone, RecordedLocation},
    position::{Coordinates, GridLength, Locator, Position},
    scalar::{Hertz, RecordedCallsign, UtcSecond, UtcTimestamp},
    service::{
        destination::{
            Destination, ExternalRecordKey, LotwRecordKey, RemoteCollection, RemoteRecordId,
            Service,
        },
        evidence::{
            ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim,
            download::{AcquisitionPhase, AcquisitionState, DownloadControl, StreamProgress},
            observation::{
                AcquisitionProvenance, ConfirmationStatus, ContactReport, ModeReport,
                ObservedStart, ProviderOrdering, QueryIdentity, QueryStream, RemoteFacts,
                RemoteIdentity, RemoteObservation, Reported, StationReport,
            },
            submission::{
                ConfirmationBasis, DispatchPhase, ResultScope, SubmissionAttempt,
                SubmissionOutcome, SubmissionProjection, SubmissionResult,
            },
        },
        lotw::{LotwMappingProvenance, LotwSubmission, PublicSigningIdentity},
        qrz::QrzSubmission,
    },
};
use clacks_sync::{
    clock::{ClockActor, RecordVector},
    record::{Position as SyncPosition, RecordState, RecordValue, VersionStamp},
};

fn id<T: std::str::FromStr>(value: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(value).to_string().parse().unwrap()
}

fn query(destination: DestinationId) -> QueryIdentity {
    QueryIdentity {
        destination,
        stream: QueryStream::LotwConfirmation,
        selected_fields: ["CALL".into(), "LOTW_QSLRDATE".into()].into(),
        own_call: Some(RecordedCallsign::new("W7HEX/P").unwrap()),
        since: Some(UtcSecond::from_unix_seconds(-100)),
    }
}

fn lotw_key() -> LotwRecordKey {
    LotwRecordKey {
        own_call: RecordedCallsign::new("W7HEX/P").unwrap(),
        worked_call: RecordedCallsign::new("K1ABC").unwrap(),
        start: UtcSecond::from_unix_seconds(600),
        band: "20M".into(),
        receive_band: "2M".into(),
        mode: "DATA".into(),
    }
}

#[tokio::test]
async fn service_values_preserve_all_native_variants_and_support() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let lotw = Destination {
        id: id(100),
        collection: RemoteCollection::Lotw {
            account: "public account".into(),
        },
        description: "Retained collection".into(),
    };
    let qrz = Destination {
        id: id(101),
        collection: RemoteCollection::Qrz {
            book_id: Some("18446744073709551616000".parse()?),
        },
        description: "QRZ".into(),
    };
    let external = ExternalRecordKey {
        destination: qrz.id,
        record: RemoteRecordId::Qrz("18446744073709551616001".parse()?),
    };
    let external_lotw = ExternalRecordKey {
        destination: lotw.id,
        record: RemoteRecordId::Lotw(lotw_key()),
    };
    let stamp = VersionStamp {
        accepted_at: SyncPosition {
            timeline: id(2),
            revision: 42,
        },
        baseline: id(3),
        vector: RecordVector::from_entries([
            (ClockActor::Server(id(2)), 42),
            (ClockActor::Client(id(9)), 7),
        ])?,
    };
    let support = ServiceSupport {
        required: vec![
            SupportRequirement::ExactFact(VersionReference {
                key: RecordKey {
                    account: id(1),
                    id: RecordId::Qso(id(10)),
                },
                stamp: stamp.clone(),
                deleted: false,
            }),
            SupportRequirement::CoveredEffect(VersionReference {
                key: RecordKey {
                    account: id(1),
                    id: RecordId::Tag(id(11)),
                },
                stamp: stamp.clone(),
                deleted: true,
            }),
        ],
        operation: Some(id(900)),
        acquisition: Some(stamp),
        destination: Some(lotw.clone()),
    };
    let at = UtcTimestamp::new(1000, 123456789)?;
    let lotw_projection = SubmissionProjection::Lotw(Box::new(LotwSubmission {
        mapping_provenance: LotwMappingProvenance {
            release: "historic".into(),
            upstream_commit: "pinned-source".into(),
        },
        destination: lotw.id,
        signing_identity: PublicSigningIdentity {
            callsign: RecordedCallsign::new("W7HEX/P")?,
            dxcc: "291".into(),
        },
        contact_fields: [
            ("MODE".into(), "DATA".into()),
            ("FREQ".into(), "14.074".into()),
        ]
        .into(),
        station_fields: [("US_STATE".into(), "WA".into())].into(),
        key: lotw_key(),
    }));
    let mut newer_mapping = lotw_projection.clone();
    if let SubmissionProjection::Lotw(value) = &mut newer_mapping {
        value.mapping_provenance.release = "different metadata".into();
    }
    assert!(lotw_projection.same_payload(&newer_mapping));
    assert_ne!(lotw_projection, newer_mapping);
    let mut values = vec![
        ServicePayload::Destination(lotw.clone()),
        ServicePayload::Destination(qrz.clone()),
        ServicePayload::Destination(Destination {
            id: id(102),
            collection: RemoteCollection::Qrz { book_id: None },
            description: "Unknown book".into(),
        }),
        ServicePayload::Imported(Box::new(ImportedAssertion {
            id: id(110),
            qso: id(10),
            service: Service::Lotw,
            destination: None,
            claim: ImportedClaim::Uploaded,
            date: Some(UtcSecond::from_unix_seconds(700)),
            provenance: ImportProvenance {
                source: "Import".into(),
                record: "row 12".into(),
                mapping: "source mapping".into(),
            },
            baseline: ImportedBaseline {
                native_end: Some(UtcSecond::from_unix_seconds(601)),
                service: Service::Lotw,
                fields: [("MODE".into(), "DATA".into())].into(),
                own_location: Some(RecordedLocation {
                    position: Some(Position::ExplicitPoint {
                        coordinates: Coordinates::from_microdegrees(-90000000, 180000000)?,
                        grid_length: GridLength::new(12)?,
                    }),
                    dxcc: Some("291".into()),
                    ..Default::default()
                }),
                confirmation: ConfirmationBasis {
                    own_call: Some(RecordedCallsign::new("W7HEX/P")?),
                    worked_call: Some(RecordedCallsign::new("K1ABC")?),
                    start: Some(UtcSecond::from_unix_seconds(600)),
                    frequency: Some(Hertz::new(14074000)?),
                    mode: Some(ModeReport {
                        concrete: "FT8".into(),
                        parent: Some("MFSK".into()),
                        group: Some("DATA".into()),
                    }),
                    uninterpreted_receive_band: Some("unknown".into()),
                    ..Default::default()
                },
                coverage: ["CALL".into(), "MODE".into()].into(),
            },
        })),
        ServicePayload::Attempt(SubmissionAttempt {
            id: id(120),
            qso: id(10),
            projection: lotw_projection,
            phase: DispatchPhase::Dispatched,
            at,
            batch: Some(id(800)),
        }),
        ServicePayload::Attempt(SubmissionAttempt {
            id: id(121),
            qso: id(10),
            projection: newer_mapping,
            phase: DispatchPhase::PossibleSend,
            at,
            batch: None,
        }),
        ServicePayload::Attempt(SubmissionAttempt {
            id: id(122),
            qso: id(10),
            projection: SubmissionProjection::Qrz(QrzSubmission {
                destination: qrz.id,
                fields: [("CALL".into(), "K1ABC".into())].into(),
            }),
            phase: DispatchPhase::Intent,
            at,
            batch: None,
        }),
        ServicePayload::Result(SubmissionResult {
            id: id(130),
            destination: qrz.id,
            scope: ResultScope::UploadBatch {
                batch: id(801),
                members: [id(122), id(123)].into(),
            },
            outcome: SubmissionOutcome::Success,
            phase: DispatchPhase::Dispatched,
            at,
            diagnostic: Some("accepted request".into()),
            external: Some(external.clone()),
            external_attempt: Some(id(122)),
        }),
        ServicePayload::Suppression(DeletionSuppression {
            id: id(150),
            qso: id(10),
            external: external_lotw,
        }),
        ServicePayload::Download(DownloadControl {
            id: lotw.id.to_string().parse()?,
            destination: lotw.id,
            active: Some(id(160)),
            progress: vec![StreamProgress {
                query: query(lotw.id),
                watermark: Some(UtcSecond::from_unix_seconds(800)),
            }],
        }),
        ServicePayload::Acquisition(AcquisitionState {
            id: id(160),
            query: query(lotw.id),
            phase: AcquisitionPhase::Completed,
            started_at: at,
            finished_at: Some(UtcTimestamp::new(1001, 999999999)?),
            complete: true,
            retained_inputs: [id(170), id(171)].into(),
        }),
    ];
    for (offset, outcome) in [
        SubmissionOutcome::Rejected("reason".into()),
        SubmissionOutcome::Uncertain("interrupted".into()),
    ]
    .into_iter()
    .enumerate()
    {
        values.push(ServicePayload::Result(SubmissionResult {
            id: id(131 + offset as u128),
            destination: qrz.id,
            scope: ResultScope::IndividualAttempt(id(122)),
            outcome,
            phase: DispatchPhase::PossibleSend,
            at,
            diagnostic: None,
            external: None,
            external_attempt: None,
        }));
    }
    for (offset, state) in [
        AssociationState::Active,
        AssociationState::Superseded,
        AssociationState::DeletedQso,
    ]
    .into_iter()
    .enumerate()
    {
        values.push(ServicePayload::Association(ExternalAssociation {
            id: id(140 + offset as u128),
            external: external.clone(),
            qso: id(10),
            state,
        }));
    }
    for (offset, decision) in [
        ProcessingDecision::PendingReview,
        ProcessingDecision::Associated,
        ProcessingDecision::Added,
        ProcessingDecision::Rejected,
        ProcessingDecision::Suppressed,
        ProcessingDecision::CompletedEquivalent,
    ]
    .into_iter()
    .enumerate()
    {
        values.push(ServicePayload::Disposition(ProcessingDisposition {
            id: id(180 + offset as u128),
            observation: id(170),
            decision,
            association: Some(id(140)),
        }));
    }
    for (offset, counterpart) in [None, Some(StationReport::default())]
        .into_iter()
        .enumerate()
    {
        values.push(ServicePayload::Observation(Box::new(RemoteObservation {
            id: id(170 + offset as u128),
            destination: lotw.id,
            identity: RemoteIdentity::Incomplete {
                service: Service::Lotw,
                available: [("CALL".into(), "K1ABC".into())].into(),
            },
            facts: RemoteFacts {
                contact: ContactReport {
                    own_call: Reported::Value(RecordedCallsign::new("W7HEX/P")?),
                    worked_call: Reported::Clear,
                    start: Reported::Value(ObservedStart::Minute(UtcSecond::from_unix_seconds(
                        600,
                    ))),
                    band: Reported::Value("20M".into()),
                    receive_band: Reported::NotReported,
                    frequency: Reported::Value(Hertz::new(14074000)?),
                    receive_frequency: Reported::Clear,
                    mode: Reported::Value(ModeReport {
                        concrete: "FT8".into(),
                        parent: Some("MFSK".into()),
                        group: Some("DATA".into()),
                    }),
                    confirmation: Reported::Value(ConfirmationStatus::Confirmed),
                },
                own_station: StationReport {
                    name: Reported::Clear,
                    position: Reported::Value(Position::GridArea(Locator::parse("CN")?)),
                    dxcc: Reported::Value("291".into()),
                    primary_subdivision: Reported::NotReported,
                    secondary_subdivision: Reported::Value("King".into()),
                    cq_zone: Reported::Value(CqZone::new(3)?),
                    itu_zone: Reported::Clear,
                    references: Reported::Value(Default::default()),
                },
                counterpart,
            },
            ordering: ProviderOrdering {
                qso: Some(UtcSecond::from_unix_seconds(900)),
                confirmation: Some(UtcSecond::from_unix_seconds(901)),
            },
            acquisition: AcquisitionProvenance {
                id: id(160),
                query: query(lotw.id),
                observed_at: at,
            },
        })));
    }
    let mut complete = values
        .iter()
        .find_map(|value| match value {
            ServicePayload::Observation(value) => Some((**value).clone()),
            _ => None,
        })
        .unwrap();
    complete.id = id(172);
    complete.destination = qrz.id;
    complete.identity = RemoteIdentity::Complete(external.clone());
    complete.acquisition.query.destination = qrz.id;
    complete.acquisition.query.stream = QueryStream::Qrz;
    complete.facts.contact.start =
        Reported::Value(ObservedStart::Second(UtcSecond::from_unix_seconds(601)));
    complete.facts.contact.confirmation = Reported::Value(ConfirmationStatus::Unconfirmed);
    complete.facts.own_station.position = Reported::Value(Position::ExplicitPoint {
        coordinates: Coordinates::from_microdegrees(-90000000, -180000000)?,
        grid_length: GridLength::new(2)?,
    });
    complete.facts.own_station.references = Reported::Value(
        [
            clacks_model::reference::ReferenceAssociation {
                group_id: id(30),
                identifier: None,
            },
            clacks_model::reference::ReferenceAssociation {
                group_id: id(30),
                identifier: Some("one".into()),
            },
        ]
        .into(),
    );
    complete.facts.counterpart = Some(StationReport {
        references: Reported::Clear,
        name: Reported::Value(String::new()),
        ..Default::default()
    });
    values.push(ServicePayload::Observation(Box::new(complete)));
    for (n, phase, finished_at) in [
        (161, AcquisitionPhase::Active, None),
        (162, AcquisitionPhase::Failed("cancelled".into()), Some(at)),
    ] {
        values.push(ServicePayload::Acquisition(AcquisitionState {
            id: id(n),
            query: query(lotw.id),
            phase,
            started_at: at,
            finished_at,
            complete: false,
            retained_inputs: Default::default(),
        }));
    }
    let mut confirmed = values
        .iter()
        .find_map(|value| match value {
            ServicePayload::Imported(value) => Some((**value).clone()),
            _ => None,
        })
        .unwrap();
    confirmed.id = id(111);
    confirmed.claim = ImportedClaim::Confirmed;
    confirmed.date = None;
    confirmed.baseline.own_location = Some(RecordedLocation::default());
    values.push(ServicePayload::Imported(Box::new(confirmed)));
    let mut tx = store.write().await?;
    let first_key = identity::external(&mut tx, id(1), &external).await?;
    assert_eq!(
        first_key,
        identity::external(&mut tx, id(1), &external).await?
    );
    let mut other_destination = external.clone();
    other_destination.destination = id(103);
    assert_ne!(
        first_key,
        identity::external(&mut tx, id(1), &other_destination).await?
    );
    let exact = ExternalRecordKey {
        destination: lotw.id,
        record: RemoteRecordId::Lotw(lotw_key()),
    };
    let first_key = identity::external(&mut tx, id(1), &exact).await?;
    let mut near = lotw_key();
    near.start = UtcSecond::from_unix_seconds(601);
    assert_ne!(
        first_key,
        identity::external(
            &mut tx,
            id(1),
            &ExternalRecordKey {
                destination: lotw.id,
                record: RemoteRecordId::Lotw(near)
            }
        )
        .await?
    );
    let mut saved = Vec::new();
    for payload in values {
        let mut support = support.clone();
        if payload.destination() == Some(qrz.id) {
            support.destination = Some(qrz.clone());
        }
        if payload.destination() == Some(id(102)) {
            support.destination = None;
        }
        let value = RecordValue::Service(Box::new(ServiceRecord { payload, support }));
        let key = RecordKey {
            account: id(1),
            id: value.id(),
        };
        let state = RecordState::Live(value);
        let root = crate::payload::insert(&mut tx, key, &state).await?;
        assert_eq!(crate::payload::load(&mut tx, key, root).await?, state);
        let wrong = RecordKey {
            account: id(2),
            ..key
        };
        assert!(crate::payload::load(&mut tx, wrong, root).await.is_err());
        saved.push((key, root, state));
    }
    tx.commit().await?;
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let (destination_key, destination_root, _) = saved
        .iter()
        .find(|(key, _, _)| matches!(key.id, RecordId::Service(ServiceRecordId::Destination(_))))
        .unwrap()
        .clone();
    let (_, observation_root, _) = saved
        .iter()
        .find(|(key, _, _)| matches!(key.id, RecordId::Service(ServiceRecordId::Observation(_))))
        .unwrap()
        .clone();
    let mut connection = store.pool.acquire().await?;
    for (key, root, state) in saved {
        assert_eq!(
            crate::payload::load(&mut connection, key, root).await?,
            state
        );
    }
    drop(connection);
    let mut tx = store.write().await?;
    assert!(
        sqlx::query_file!("tests/sql/invalid_report_frequency.sql", observation_root)
            .execute(&mut *tx)
            .await
            .is_err()
    );
    sqlx::query_file!("tests/sql/orphan_service_map.sql", destination_root)
        .execute(&mut *tx)
        .await?;
    assert!(
        crate::payload::load(&mut tx, destination_key, destination_root)
            .await
            .is_err()
    );
    tx.rollback().await?;
    store.close().await?;
    Ok(())
}
