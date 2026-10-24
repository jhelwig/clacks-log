//! Immutable relational values shared by accepted and unpublished roles.

pub(crate) mod contact;
mod definitions;
mod service;
mod shared;

use clacks_model::service::evidence::ServiceRecordId;
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use sqlx::SqliteConnection;

/// Select the relational discriminator for a native identity.
pub(crate) fn kind(id: RecordId) -> &'static str {
    match id {
        RecordId::Qso(_) => "qso",
        RecordId::Tag(_) => "tag",
        RecordId::Profile(_) => "profile",
        RecordId::Equipment(_) => "equipment",
        RecordId::Group(_) => "group",
        RecordId::OutputProfile(_) => "output_profile",
        RecordId::Service(service) => match service {
            ServiceRecordId::Destination(_) => "destination",
            ServiceRecordId::Assertion(_) => "imported",
            ServiceRecordId::Attempt(_) => "attempt",
            ServiceRecordId::Result(_) => "result",
            ServiceRecordId::Observation(_) => "observation",
            ServiceRecordId::Association(_) => "association",
            ServiceRecordId::Disposition(_) => "disposition",
            ServiceRecordId::Suppression(_) => "suppression",
            ServiceRecordId::Download(_) => "download",
            ServiceRecordId::Acquisition(_) => "acquisition",
        },
    }
}

/// Decode a stored native identity while retaining its kind and service subtype.
pub(crate) fn record_id(kind: &str, value: &str) -> anyhow::Result<RecordId> {
    Ok(match kind {
        "qso" => RecordId::Qso(value.parse()?),
        "tag" => RecordId::Tag(value.parse()?),
        "profile" => RecordId::Profile(value.parse()?),
        "equipment" => RecordId::Equipment(value.parse()?),
        "group" => RecordId::Group(value.parse()?),
        "output_profile" => RecordId::OutputProfile(value.parse()?),
        "destination" => RecordId::Service(ServiceRecordId::Destination(value.parse()?)),
        "imported" => RecordId::Service(ServiceRecordId::Assertion(value.parse()?)),
        "attempt" => RecordId::Service(ServiceRecordId::Attempt(value.parse()?)),
        "result" => RecordId::Service(ServiceRecordId::Result(value.parse()?)),
        "observation" => RecordId::Service(ServiceRecordId::Observation(value.parse()?)),
        "association" => RecordId::Service(ServiceRecordId::Association(value.parse()?)),
        "disposition" => RecordId::Service(ServiceRecordId::Disposition(value.parse()?)),
        "suppression" => RecordId::Service(ServiceRecordId::Suppression(value.parse()?)),
        "download" => RecordId::Service(ServiceRecordId::Download(value.parse()?)),
        "acquisition" => RecordId::Service(ServiceRecordId::Acquisition(value.parse()?)),
        _ => anyhow::bail!("unknown stored native record kind"),
    })
}

/// Ensure a structurally referenced identity without claiming its live existence.
pub(crate) async fn identity(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<()> {
    let account = key.account.to_string();
    let discriminator = kind(key.id);
    let id = key.id.to_string();
    sqlx::query_file!("sql/payload/identity.sql", account, discriminator, id)
        .execute(connection)
        .await?;
    Ok(())
}

/// Store one validated immutable payload in the caller's transaction.
pub(crate) async fn insert(
    connection: &mut SqliteConnection,
    key: RecordKey,
    state: &RecordState,
) -> anyhow::Result<i64> {
    state.validate(key)?;
    identity(connection, key).await?;
    let account = key.account.to_string();
    let discriminator = kind(key.id);
    let id = key.id.to_string();
    let deleted = matches!(state, RecordState::Deleted);
    let row = sqlx::query_file!(
        "sql/payload/insert.sql",
        account,
        discriminator,
        id,
        deleted
    )
    .fetch_one(&mut *connection)
    .await?;
    match state {
        RecordState::Deleted => {}
        RecordState::Live(RecordValue::Qso(qso)) => {
            contact::insert(connection, key.account, row.payload_id, qso).await?
        }
        RecordState::Live(RecordValue::Group(group)) => {
            definitions::insert(connection, key.account, row.payload_id, group).await?
        }
        RecordState::Live(RecordValue::Service(value)) => {
            service::insert(connection, key, row.payload_id, value).await?
        }
        RecordState::Live(value) => shared::insert(connection, key, row.payload_id, value).await?,
    }
    Ok(row.payload_id)
}

/// Restore a qualified payload through native validation without recognition changes.
pub(crate) async fn load(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<RecordState> {
    let row = sqlx::query_file!("sql/payload/select.sql", payload)
        .fetch_one(&mut *connection)
        .await?;
    anyhow::ensure!(
        row.account == key.account.to_string()
            && row.kind == kind(key.id)
            && row.record_id == key.id.to_string(),
        "payload identity differs from requested record"
    );
    let roots = sqlx::query_file!("sql/payload/root_count.sql", payload)
        .fetch_one(&mut *connection)
        .await?
        .roots;
    let state = if row.deleted != 0 {
        anyhow::ensure!(row.deleted == 1, "invalid payload deletion discriminator");
        anyhow::ensure!(roots == 0, "deleted payload has a live root");
        RecordState::Deleted
    } else {
        anyhow::ensure!(roots == 1, "live payload must have exactly one native root");
        match key.id {
            RecordId::Qso(id) => RecordState::Live(RecordValue::Qso(Box::new(
                contact::load(connection, key.account, payload, id).await?,
            ))),
            RecordId::Group(id) => RecordState::Live(RecordValue::Group(Box::new(
                definitions::load(connection, key.account, payload, id).await?,
            ))),
            RecordId::Service(_) => RecordState::Live(RecordValue::Service(Box::new(
                service::load(connection, key, payload).await?,
            ))),
            _ => RecordState::Live(shared::load(connection, key, payload).await?),
        }
    };
    state.validate(key)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use crate::store::{ClientStore, StoreSettings};
    use clacks_model::{
        contact::{BandFrequency, OwnOperatingContext, Qso, StationSnapshot},
        recognition::{
            band::Band,
            mode::{Mode, ModeIdentity},
        },
        scalar::{RecordedCallsign, UtcSecond},
    };
    use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};

    fn id<T: std::str::FromStr>(n: u128) -> T
    where
        T::Err: std::fmt::Debug,
    {
        ulid::Ulid::from(n).to_string().parse().unwrap()
    }

    #[tokio::test]
    async fn shared_payloads_round_trip() -> anyhow::Result<()> {
        use clacks_model::{
            equipment::EquipmentValues,
            export::{OutputProfile, OutputPurpose},
            profile::{EquipmentConfiguration, SavedLocationProfile},
            tag::{AutomaticUploadPolicies, AutomaticUploadPolicy, LogbookTag},
        };
        let dir = tempfile::tempdir()?;
        let store = ClientStore::create(
            &dir.path().join("workspace.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let mut tx = store.write().await?;
        for value in [
            RecordValue::Tag(Box::new(LogbookTag::new(id(20), "Same name".into()))),
            RecordValue::Tag(Box::new(LogbookTag {
                id: id(23),
                name: "Same name".into(),
                automatic_upload: AutomaticUploadPolicies {
                    lotw: AutomaticUploadPolicy::Enable,
                    qrz: AutomaticUploadPolicy::Forbid,
                },
            })),
            RecordValue::Tag(Box::new(LogbookTag {
                id: id(24),
                name: "Same name".into(),
                automatic_upload: AutomaticUploadPolicies {
                    lotw: AutomaticUploadPolicy::Forbid,
                    qrz: AutomaticUploadPolicy::Enable,
                },
            })),
            RecordValue::Equipment(Box::new(EquipmentConfiguration {
                id: id(21),
                name: "Same name".into(),
                values: EquipmentValues::default(),
            })),
            RecordValue::OutputProfile(Box::new(OutputProfile {
                id: id(22),
                name: "Same name".into(),
                purpose: OutputPurpose::Targeted,
                contextual_fields: ["CONTEST_ID".into()].into(),
            })),
            RecordValue::OutputProfile(Box::new(OutputProfile {
                id: id(25),
                name: "Same name".into(),
                purpose: OutputPurpose::OrdinaryInterchange,
                contextual_fields: Default::default(),
            })),
            RecordValue::Profile(Box::new(SavedLocationProfile {
                id: id(26),
                name: "Same name".into(),
                station: StationSnapshot {
                    callsign: RecordedCallsign::new("W7HEX")?,
                    location: None,
                    references: Default::default(),
                },
                default_equipment: Some(id(99)),
            })),
            RecordValue::Profile(Box::new(SavedLocationProfile {
                id: id(27),
                name: "Same name".into(),
                station: StationSnapshot {
                    callsign: RecordedCallsign::new("W7HEX/P")?,
                    location: None,
                    references: Default::default(),
                },
                default_equipment: None,
            })),
        ] {
            let key = RecordKey {
                account: id(1),
                id: value.id(),
            };
            let state = RecordState::Live(value);
            let payload = super::insert(&mut tx, key, &state).await?;
            assert_eq!(super::load(&mut tx, key, payload).await?, state);
        }
        tx.rollback().await?;
        store.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn service_destination_round_trip() -> anyhow::Result<()> {
        use clacks_model::service::{
            destination::{Destination, RemoteCollection},
            evidence::ServicePayload,
        };
        use clacks_sync::service::{ServiceRecord, ServiceSupport};
        let dir = tempfile::tempdir()?;
        let store = ClientStore::create(
            &dir.path().join("workspace.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let value = RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Destination(Destination {
                id: id(100),
                collection: RemoteCollection::Qrz {
                    book_id: Some("18446744073709551616000".parse()?),
                },
                description: "Historical namespace".into(),
            }),
            support: ServiceSupport::default(),
        }));
        let key = RecordKey {
            account: id(1),
            id: value.id(),
        };
        let state = RecordState::Live(value);
        let mut tx = store.write().await?;
        let root = super::insert(&mut tx, key, &state).await?;
        assert_eq!(super::load(&mut tx, key, root).await?, state);
        tx.rollback().await?;
        store.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn recorded_contact_round_trip() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = ClientStore::create(
            &dir.path().join("workspace.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let station = StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX/P")?,
            location: None,
            references: Default::default(),
        };
        let qso = Qso {
            id: id(10),
            start: UtcSecond::from_unix_seconds(-1),
            end: Some(UtcSecond::from_unix_seconds(1)),
            own_station: station.clone(),
            contacted_station: station,
            own_context: OwnOperatingContext::default(),
            contacted_operator: None,
            primary: BandFrequency::from_recorded_pair(
                None,
                Some(Band::from_recorded_label("future-band")?),
            )?,
            receive_override: None,
            mode: Mode::from_recorded_identity(ModeIdentity::Specialization {
                label: "future-submode".into(),
                parent: None,
            })?,
            sent_report: Some("+03".into()),
            received_report: Some("599".into()),
            comment: None,
            tags: Default::default(),
        };
        let key = RecordKey {
            account: id(1),
            id: RecordId::Qso(qso.id),
        };
        let state = RecordState::Live(RecordValue::Qso(Box::new(qso.clone())));
        let mut tx = store.write().await?;
        let payload = super::insert(&mut tx, key, &state).await?;
        assert_eq!(super::load(&mut tx, key, payload).await?, state);
        use clacks_model::{
            contact::OperatorInformation,
            equipment::EquipmentValues,
            location::RecordedLocation,
            position::{Coordinates, GridLength, Locator, Position},
            reference::ReferenceAssociation,
            scalar::{Hertz, Milliwatts},
        };
        let positions =
            [
                "CN",
                "CN87",
                "CN87AA",
                "CN87AA00",
                "CN87AA00AA",
                "CN87AA00AA00",
            ]
            .into_iter()
            .map(|value| Position::GridArea(Locator::parse(value).unwrap()))
            .chain([-180_000_000, 180_000_000].into_iter().map(|longitude| {
                Position::ExplicitPoint {
                    coordinates: Coordinates::from_microdegrees(90_000_000, longitude).unwrap(),
                    grid_length: GridLength::new(12).unwrap(),
                }
            }));
        let mut payloads = vec![(payload, state)];
        for position in positions {
            let mut qso = qso.clone();
            qso.own_station.location = Some(RecordedLocation {
                position: Some(position.clone()),
                dxcc: Some("291".into()),
                ..Default::default()
            });
            qso.contacted_station.location = Some(RecordedLocation {
                position: Some(position),
                secondary_subdivision: Some("King".into()),
                ..Default::default()
            });
            qso.own_station.references = [
                ReferenceAssociation {
                    group_id: id(30),
                    identifier: None,
                },
                ReferenceAssociation {
                    group_id: id(30),
                    identifier: Some("ABC-1".into()),
                },
                ReferenceAssociation {
                    group_id: id(30),
                    identifier: Some("ABC-2".into()),
                },
            ]
            .into();
            qso.contacted_station.references = qso.own_station.references.clone();
            qso.own_context.operator = Some(OperatorInformation {
                callsign: Some(RecordedCallsign::new("W7HEX")?),
                recorded_name: None,
            });
            qso.contacted_operator = Some(OperatorInformation {
                callsign: None,
                recorded_name: Some(" Name as recorded ".into()),
            });
            qso.own_context.equipment = Some(EquipmentValues {
                equipment_description: Some("Rig".into()),
                antenna_description: Some("Wire".into()),
                notes: Some("Unchanged".into()),
            });
            qso.own_context.transmit_power = Some(Milliwatts::new(100_000)?);
            qso.primary = BandFrequency::from_recorded_pair(
                Some(Hertz::new(14_074_000)?),
                Some(Band::from_recorded_label("old label")?),
            )?;
            qso.receive_override = Some(BandFrequency::from_recorded_pair(
                None,
                Some(Band::from_recorded_label("20m")?),
            )?);
            qso.tags = [id(40), id(41)].into();
            let state = RecordState::Live(RecordValue::Qso(Box::new(qso)));
            let payload = super::insert(&mut tx, key, &state).await?;
            assert_eq!(super::load(&mut tx, key, payload).await?, state);
            let group = id::<clacks_model::ids::GroupId>(30).to_string();
            assert!(
                sqlx::query_file!("tests/sql/duplicate_group_only.sql", group, payload)
                    .execute(&mut *tx)
                    .await
                    .is_err()
            );
            payloads.push((payload, state));
        }
        let query = sqlx::query_file!("tests/sql/contact_query.sql", 14_000_000_i64, 100_000_i64)
            .fetch_all(&mut *tx)
            .await?;
        assert_eq!(query.len(), 8);
        for row in query {
            assert_eq!(
                (row.frequency, row.power, row.tags, row.references),
                (Some(14_074_000), Some(100_000), 2, 3)
            );
        }
        tx.commit().await?;
        store.close().await?;
        let store = ClientStore::open(
            &dir.path().join("workspace.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let mut tx = store.write().await?;
        for (payload, state) in payloads {
            assert_eq!(super::load(&mut tx, key, payload).await?, state);
        }
        sqlx::query_file!("tests/sql/remove_contact_root.sql", payload)
            .execute(&mut *tx)
            .await?;
        assert!(super::load(&mut tx, key, payload).await.is_err());
        tx.rollback().await?;
        store.close().await?;
        Ok(())
    }
}
