//! Typed relational mappings for recorded contacts and copied station facts.

use crate::store::rows::contact::PositionFields;
use clacks_model::{
    contact::{BandFrequency, OperatorInformation, Qso, StationSnapshot},
    equipment::EquipmentValues,
    ids::{AccountId, QsoId},
    location::{CqZone, ItuZone},
    position::{Coordinates, GridLength, Locator, Position},
    recognition::{
        band::Band,
        mode::{Mode, ModeIdentity},
    },
    reference::ReferenceAssociation,
    scalar::{Hertz, Milliwatts, RecordedCallsign, UtcSecond},
};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::PgConnection;

/// Restore a callsign while rejecting noncanonical database contents instead of rewriting them.
fn callsign(value: String) -> anyhow::Result<RecordedCallsign> {
    let parsed = RecordedCallsign::new(&value)?;
    anyhow::ensure!(parsed.as_str() == value, "stored callsign is not canonical");
    Ok(parsed)
}

/// Restore an independently optional operator identity and recorded name.
fn operator(
    call: Option<String>,
    name: Option<String>,
) -> anyhow::Result<Option<OperatorInformation>> {
    if call.is_none() && name.is_none() {
        return Ok(None);
    }
    Ok(Some(
        (clacks_model::contact::OperatorInformationFields {
            callsign: call.map(callsign).transpose()?,
            recorded_name: name,
        })
        .try_into()?,
    ))
}

/// Store an owned equipment copy without a live configuration reference.
pub(crate) async fn insert_equipment(
    connection: &mut PgConnection,
    account: AccountId,
    value: &EquipmentValues,
) -> anyhow::Result<i64> {
    let account = account.to_string();
    Ok(sqlx::query_file!(
        "sql/persistence/equipment/snapshot_insert.sql",
        account,
        value.equipment_description().as_deref(),
        value.antenna_description().as_deref(),
        value.notes().as_deref()
    )
    .fetch_one(connection)
    .await?
    .equipment_id)
}

/// Restore copied equipment fields without consulting reusable configuration records.
pub(crate) async fn load_equipment(
    connection: &mut PgConnection,
    account: AccountId,
    equipment: i64,
) -> anyhow::Result<EquipmentValues> {
    let account = account.to_string();
    let row = sqlx::query_file!(
        "sql/persistence/equipment/snapshot_select.sql",
        equipment,
        account
    )
    .fetch_one(connection)
    .await?;
    let value = clacks_model::equipment::EquipmentValuesFields {
        equipment_description: row.equipment_description,
        antenna_description: row.antenna_description,
        notes: row.notes,
    };
    (value).try_into()
}

/// Store a station snapshot and its recorded location and unordered reference set.
pub(crate) async fn insert_station(
    connection: &mut PgConnection,
    account: AccountId,
    station: &StationSnapshot,
) -> anyhow::Result<i64> {
    let account_text = account.to_string();
    let call = station.callsign.as_str();
    let id = sqlx::query_file!("sql/persistence/station/insert.sql", account_text, call)
        .fetch_one(&mut *connection)
        .await?
        .station_id;
    if let Some(location) = &station.location {
        let position =
            crate::store::rows::contact::PositionFields::from(location.position().as_ref());

        let cq = location.cq_zone().map(|zone| i64::from(zone.get()));
        let itu = location.itu_zone().map(|zone| i64::from(zone.get()));
        sqlx::query_file!(
            "sql/persistence/location/insert.sql",
            id,
            account_text,
            position.kind,
            position.locator,
            position.latitude,
            position.longitude,
            position.length,
            location.dxcc().as_deref(),
            location.primary_subdivision().as_deref(),
            location.secondary_subdivision().as_deref(),
            cq,
            itu
        )
        .execute(&mut *connection)
        .await?;
    }
    for reference in &station.references {
        super::identity(
            connection,
            RecordKey {
                account,
                id: RecordId::Group(reference.group_id),
            },
        )
        .await?;
        let group = reference.group_id.to_string();
        sqlx::query_file!(
            "sql/persistence/station/reference_insert.sql",
            id,
            account_text,
            group,
            reference.identifier().as_deref()
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(id)
}

/// Restore a station's exact recorded position form and reference associations.
pub(crate) async fn load_station(
    connection: &mut PgConnection,
    account: AccountId,
    station: i64,
) -> anyhow::Result<StationSnapshot> {
    let account_text = account.to_string();
    let row = sqlx::query_file!("sql/persistence/station/select.sql", station, account_text)
        .fetch_one(&mut *connection)
        .await?;
    let location = if let Some(row) =
        sqlx::query_file!("sql/persistence/location/select.sql", station, account_text)
            .fetch_optional(&mut *connection)
            .await?
    {
        let position = match (PositionFields {
            kind: row.position_kind,
            locator: row.locator,
            latitude: row.latitude,
            longitude: row.longitude,
            length: row.grid_length,
        }) {
            PositionFields {
                kind: None,
                locator: None,
                latitude: None,
                longitude: None,
                length: None,
            } => None,
            PositionFields {
                kind: Some(kind),
                locator: Some(grid),
                latitude: None,
                longitude: None,
                length: None,
            } if kind == "grid" => Some(Position::GridArea(Locator::parse(&grid)?)),
            PositionFields {
                kind: Some(kind),
                locator: None,
                latitude: Some(latitude),
                longitude: Some(longitude),
                length: Some(length),
            } if kind == "point" => Some(Position::ExplicitPoint {
                coordinates: Coordinates::from_microdegrees(
                    latitude.try_into()?,
                    longitude.try_into()?,
                )?,
                grid_length: GridLength::new(length.try_into()?)?,
            }),
            _ => anyhow::bail!("invalid recorded position shape"),
        };
        Some(clacks_model::location::RecordedLocationFields {
            position,
            dxcc: row.dxcc,
            primary_subdivision: row.primary_subdivision,
            secondary_subdivision: row.secondary_subdivision,
            cq_zone: row
                .cq_zone
                .map(|zone| Ok::<_, anyhow::Error>(CqZone::new(zone.try_into()?)?))
                .transpose()?,
            itu_zone: row
                .itu_zone
                .map(|zone| Ok::<_, anyhow::Error>(ItuZone::new(zone.try_into()?)?))
                .transpose()?,
        })
    } else {
        None
    };
    let mut references = std::collections::BTreeSet::new();
    for reference in sqlx::query_file!(
        "sql/persistence/station/references.sql",
        station,
        account_text
    )
    .fetch_all(&mut *connection)
    .await?
    {
        anyhow::ensure!(
            references.insert(ReferenceAssociation::new(
                reference.group_id.parse()?,
                reference.identifier
            )?),
            "duplicate stored station reference"
        );
    }
    let station = clacks_model::contact::StationSnapshot {
        callsign: callsign(row.callsign)?,
        location: location.map(TryInto::try_into).transpose()?,
        references,
    };
    Ok(station)
}

/// Store a complete validated QSO and copied child values in the existing transaction.
pub(super) async fn insert(
    connection: &mut PgConnection,
    account: AccountId,
    payload: i64,
    qso: &Qso,
) -> anyhow::Result<()> {
    let own = insert_station(connection, account, qso.own_station()).await?;
    let contacted = insert_station(connection, account, qso.contacted_station()).await?;
    let equipment = match &qso.own_context().equipment {
        Some(value) => Some(insert_equipment(connection, account, value).await?),
        None => None,
    };
    let account_text = account.to_string();
    let id = qso.id().to_string();
    let start = qso.start().unix_seconds();
    let end = qso.end().map(UtcSecond::unix_seconds);
    let own_call = qso
        .own_context()
        .operator
        .as_ref()
        .and_then(|operator| operator.callsign().as_ref())
        .map(RecordedCallsign::as_str);
    let own_name = qso
        .own_context()
        .operator
        .as_ref()
        .and_then(|operator| operator.recorded_name().as_deref());
    let contacted_call = qso
        .contacted_operator()
        .as_ref()
        .and_then(|operator| operator.callsign().as_ref())
        .map(RecordedCallsign::as_str);
    let contacted_name = qso
        .contacted_operator()
        .as_ref()
        .and_then(|operator| operator.recorded_name().as_deref());
    let power = qso
        .own_context()
        .transmit_power
        .map(|value| i64::try_from(value.get()))
        .transpose()?;
    let frequency = qso
        .primary()
        .frequency()
        .map(|value| i64::try_from(value.get()))
        .transpose()?;
    let band = qso.primary().band().map(Band::label);
    let receive_frequency = qso
        .receive_override()
        .as_ref()
        .and_then(BandFrequency::frequency)
        .map(|value| i64::try_from(value.get()))
        .transpose()?;
    let receive_band = qso
        .receive_override()
        .as_ref()
        .and_then(BandFrequency::band)
        .map(Band::label);
    match qso.mode().identity() {
        ModeIdentity::Parent { label } => {
            sqlx::query_file!(
                "sql/persistence/contact/insert.sql",
                payload,
                account_text,
                id,
                start,
                end,
                own,
                contacted,
                own_call,
                own_name,
                contacted_call,
                contacted_name,
                power,
                equipment,
                frequency,
                band,
                receive_frequency,
                receive_band,
                "parent",
                label,
                None::<String>,
                qso.sent_report().as_deref(),
                qso.received_report().as_deref(),
                qso.comment().as_deref()
            )
            .execute(&mut *connection)
            .await?;
        }
        ModeIdentity::Specialization { label, parent } => {
            sqlx::query_file!(
                "sql/persistence/contact/insert.sql",
                payload,
                account_text,
                id,
                start,
                end,
                own,
                contacted,
                own_call,
                own_name,
                contacted_call,
                contacted_name,
                power,
                equipment,
                frequency,
                band,
                receive_frequency,
                receive_band,
                "specialization",
                label,
                parent,
                qso.sent_report().as_deref(),
                qso.received_report().as_deref(),
                qso.comment().as_deref()
            )
            .execute(&mut *connection)
            .await?;
        }
    }
    for tag in qso.tags() {
        super::identity(
            connection,
            RecordKey {
                account,
                id: RecordId::Tag(*tag),
            },
        )
        .await?;
        let tag = tag.to_string();
        sqlx::query_file!(
            "sql/persistence/contact/tag_insert.sql",
            payload,
            account_text,
            tag
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Restore native quantities from signed columns without truncation or reinterpretation.
fn pair(frequency: Option<i64>, band: Option<String>) -> anyhow::Result<BandFrequency> {
    BandFrequency::from_recorded_pair(
        frequency
            .map(|value| Ok::<_, anyhow::Error>(Hertz::new(value.try_into()?)?))
            .transpose()?,
        band.map(|value| Band::from_recorded_label(&value))
            .transpose()?,
    )
}

/// Restore one QSO payload without changing recorded labels or materializing fallback facts.
pub(super) async fn load(
    connection: &mut PgConnection,
    account: AccountId,
    payload: i64,
    id: QsoId,
) -> anyhow::Result<Qso> {
    let account_text = account.to_string();
    let id_text = id.to_string();
    let row = sqlx::query_file!(
        "sql/persistence/contact/select.sql",
        payload,
        account_text,
        id_text
    )
    .fetch_one(&mut *connection)
    .await?;
    let own_station = load_station(connection, account, row.own_station).await?;
    let contacted_station = load_station(connection, account, row.contacted_station).await?;
    let equipment = match row.equipment_id {
        Some(id) => Some(load_equipment(connection, account, id).await?),
        None => None,
    };
    let mode = match row.mode_kind.as_str() {
        "parent" if row.mode_parent.is_none() => ModeIdentity::Parent {
            label: row.mode_label,
        },
        "specialization" => ModeIdentity::Specialization {
            label: row.mode_label,
            parent: row.mode_parent,
        },
        _ => anyhow::bail!("invalid recorded mode shape"),
    };
    let receive_override = if row.receive_frequency.is_none() && row.receive_band.is_none() {
        None
    } else {
        Some(pair(row.receive_frequency, row.receive_band)?)
    };
    let mut tags = std::collections::BTreeSet::new();
    for tag in sqlx::query_file!("sql/persistence/contact/tags.sql", payload, account_text)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(tags.insert(tag.tag_id.parse()?), "duplicate stored QSO tag");
    }
    let fields = clacks_model::contact::input::QsoFields {
        id,
        start: UtcSecond::from_unix_seconds(row.start),
        end: row.end.map(UtcSecond::from_unix_seconds),
        own_station,
        contacted_station,
        own_context: clacks_model::contact::OwnOperatingContext {
            operator: (operator(row.own_operator_call, row.own_operator_name)?),
            transmit_power: row
                .power
                .map(|value| Ok::<_, anyhow::Error>(Milliwatts::new(value.try_into()?)?))
                .transpose()?,
            equipment,
        },
        contacted_operator: (operator(row.contacted_operator_call, row.contacted_operator_name)?),
        primary: pair(row.frequency, row.band)?,
        receive_override,
        mode: Mode::from_recorded_identity(mode)?,
        sent_report: row.sent_report,
        received_report: row.received_report,
        comment: row.comment,
        tags,
    };
    Qso::try_from(fields)
}
