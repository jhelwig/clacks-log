//! Typed provider field coverage, including explicit clears and empty reference sets.

use crate::rows::contact::PositionFields;
use clacks_model::{
    ids::AccountId,
    location::{CqZone, ItuZone},
    position::{Coordinates, GridLength, Locator, Position},
    reference::ReferenceAssociation,
    scalar::{Hertz, RecordedCallsign, UtcSecond},
    service::evidence::observation::{
        ConfirmationStatus, ContactReport, ModeReport, ObservedStart, Reported, StationReport,
    },
};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Decode position columns only when they form one complete native variant.
pub(super) fn position(fields: PositionFields) -> anyhow::Result<Option<Position>> {
    Ok(match fields {
        PositionFields {
            kind: None,
            locator: None,
            latitude: None,
            longitude: None,
            length: None,
        } => None,
        PositionFields {
            kind: Some(kind),
            locator: Some(locator),
            latitude: None,
            longitude: None,
            length: None,
        } if kind == "grid" => {
            let grid = Locator::parse(&locator)?;
            anyhow::ensure!(grid.to_string() == locator, "stored grid is not canonical");
            Some(Position::GridArea(grid))
        }
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
        _ => anyhow::bail!("invalid provider position columns"),
    })
}

/// Encode native report coverage without converting omission into an explicit clear.
fn split<T>(value: &Reported<T>) -> (i64, Option<&T>) {
    match value {
        Reported::NotReported => (0, None),
        Reported::Clear => (1, None),
        Reported::Value(value) => (2, Some(value)),
    }
}

/// Decode exact report coverage and reject incompatible optional payloads.
fn reported<T>(state: i64, value: Option<T>) -> anyhow::Result<Reported<T>> {
    match (state, value) {
        (0, None) => Ok(Reported::NotReported),
        (1, None) => Ok(Reported::Clear),
        (2, Some(value)) => Ok(Reported::Value(value)),
        _ => anyhow::bail!("invalid reported-field coverage or value"),
    }
}

/// Decode a positive whole-hertz quantity without truncating the stored integer.
pub(super) fn hertz(value: Option<i64>) -> anyhow::Result<Option<Hertz>> {
    value
        .map(|value| Ok(Hertz::new(value.try_into()?)?))
        .transpose()
}

/// Decode a mode report while preserving independently supplied classification.
pub(super) fn mode(
    concrete: Option<String>,
    parent: Option<String>,
    group: Option<String>,
) -> anyhow::Result<Option<ModeReport>> {
    match concrete {
        Some(concrete) => Ok(Some(ModeReport {
            concrete,
            parent,
            group,
        })),
        None if parent.is_none() && group.is_none() => Ok(None),
        None => anyhow::bail!("mode classification lacks concrete mode"),
    }
}

/// Store contact identity/status fields with their independent coverage states.
pub(super) async fn contact(
    connection: &mut SqliteConnection,
    payload: i64,
    value: &ContactReport,
) -> anyhow::Result<i64> {
    let (own_state, own) = split(&value.own_call);
    let own = own.map(RecordedCallsign::as_str);
    let (worked_state, worked) = split(&value.worked_call);
    let worked = worked.map(RecordedCallsign::as_str);
    let (start_state, start) = split(&value.start);
    let (start_kind, start) = match start {
        None => (None, None),
        Some(ObservedStart::Second(at)) => (Some("second"), Some(at.unix_seconds())),
        Some(ObservedStart::Minute(at)) => (Some("minute"), Some(at.unix_seconds())),
    };
    let (band_state, band) = split(&value.band);
    let band = band.map(String::as_str);
    let (receive_state, receive) = split(&value.receive_band);
    let receive = receive.map(String::as_str);
    let (frequency_state, frequency) = split(&value.frequency);
    let frequency = frequency
        .map(|value| i64::try_from(value.get()))
        .transpose()?;
    let (receive_frequency_state, receive_frequency) = split(&value.receive_frequency);
    let receive_frequency = receive_frequency
        .map(|value| i64::try_from(value.get()))
        .transpose()?;
    let (mode_state, mode) = split(&value.mode);
    let concrete = mode.map(|mode| mode.concrete.as_str());
    let parent = mode.and_then(|mode| mode.parent.as_deref());
    let group = mode.and_then(|mode| mode.group.as_deref());
    let (confirmation_state, confirmation) = split(&value.confirmation);
    let confirmation = confirmation.map(|value| match value {
        ConfirmationStatus::Confirmed => "confirmed",
        ConfirmationStatus::Unconfirmed => "unconfirmed",
    });
    Ok(sqlx::query_file!(
        "sql/report/contact_insert.sql",
        payload,
        own_state,
        own,
        worked_state,
        worked,
        start_state,
        start_kind,
        start,
        band_state,
        band,
        receive_state,
        receive,
        frequency_state,
        frequency,
        receive_frequency_state,
        receive_frequency,
        mode_state,
        concrete,
        parent,
        group,
        confirmation_state,
        confirmation
    )
    .fetch_one(connection)
    .await?
    .report_id)
}

/// Restore independent identity, timing and confirmation coverage from one report.
pub(super) async fn load_contact(
    connection: &mut SqliteConnection,
    payload: i64,
    report: i64,
) -> anyhow::Result<ContactReport> {
    let row = sqlx::query_file!("sql/report/contact_select.sql", report, payload)
        .fetch_one(connection)
        .await?;
    let start = match (row.start_kind.as_deref(), row.start) {
        (None, None) => None,
        (Some("second"), Some(at)) => Some(ObservedStart::Second(UtcSecond::from_unix_seconds(at))),
        (Some("minute"), Some(at)) => Some(ObservedStart::Minute(UtcSecond::from_unix_seconds(at))),
        _ => anyhow::bail!("invalid observed start columns"),
    };
    if let Some(start) = start {
        start.bounds()?;
    }
    let confirmation = row
        .confirmation
        .map(|value| match value.as_str() {
            "confirmed" => Ok(ConfirmationStatus::Confirmed),
            "unconfirmed" => Ok(ConfirmationStatus::Unconfirmed),
            _ => anyhow::bail!("unknown confirmation status"),
        })
        .transpose()?;
    Ok(ContactReport {
        own_call: reported(
            row.own_call_state,
            row.own_call
                .map(|call| RecordedCallsign::new(&call))
                .transpose()?,
        )?,
        worked_call: reported(
            row.worked_call_state,
            row.worked_call
                .map(|call| RecordedCallsign::new(&call))
                .transpose()?,
        )?,
        start: reported(row.start_state, start)?,
        band: reported(row.band_state, row.band)?,
        receive_band: reported(row.receive_band_state, row.receive_band)?,
        frequency: reported(row.frequency_state, hertz(row.frequency)?)?,
        receive_frequency: reported(row.receive_frequency_state, hertz(row.receive_frequency)?)?,
        mode: reported(
            row.mode_state,
            mode(row.mode, row.mode_parent, row.mode_group)?,
        )?,
        confirmation: reported(row.confirmation_state, confirmation)?,
    })
}

/// Store one station report, keeping value-empty reference sets distinct from omission.
pub(super) async fn station(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    value: &StationReport,
) -> anyhow::Result<i64> {
    let (name_state, name) = split(&value.name);
    let name = name.map(String::as_str);
    let (position_state, position) = split(&value.position);
    let position = PositionFields::new(position);
    let (dxcc_state, dxcc) = split(&value.dxcc);
    let dxcc = dxcc.map(String::as_str);
    let (primary_state, primary) = split(&value.primary_subdivision);
    let primary = primary.map(String::as_str);
    let (secondary_state, secondary) = split(&value.secondary_subdivision);
    let secondary = secondary.map(String::as_str);
    let (cq_state, cq) = split(&value.cq_zone);
    let cq = cq.map(|value| i64::from(value.get()));
    let (itu_state, itu) = split(&value.itu_zone);
    let itu = itu.map(|value| i64::from(value.get()));
    let (references_state, references) = split(&value.references);
    let id = sqlx::query_file!(
        "sql/report/station_insert.sql",
        payload,
        name_state,
        name,
        position_state,
        position.kind,
        position.locator,
        position.latitude,
        position.longitude,
        position.length,
        dxcc_state,
        dxcc,
        primary_state,
        primary,
        secondary_state,
        secondary,
        cq_state,
        cq,
        itu_state,
        itu,
        references_state
    )
    .fetch_one(&mut *connection)
    .await?
    .report_id;
    if let Some(references) = references {
        for reference in references {
            crate::payload::identity(
                connection,
                RecordKey {
                    account,
                    id: RecordId::Group(reference.group_id),
                },
            )
            .await?;
            let account = account.to_string();
            let group = reference.group_id.to_string();
            sqlx::query_file!(
                "sql/report/reference_insert.sql",
                id,
                account,
                group,
                reference.identifier
            )
            .execute(&mut *connection)
            .await?;
        }
    }
    Ok(id)
}

/// Restore a station report without filling omitted fields from another source.
pub(super) async fn load_station(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    report: i64,
) -> anyhow::Result<StationReport> {
    let row = sqlx::query_file!("sql/report/station_select.sql", report, payload)
        .fetch_one(&mut *connection)
        .await?;
    let account = account.to_string();
    let mut references = std::collections::BTreeSet::new();
    for reference in sqlx::query_file!("sql/report/reference_select.sql", report, account)
        .fetch_all(connection)
        .await?
    {
        anyhow::ensure!(
            references.insert(ReferenceAssociation {
                group_id: reference.group_id.parse()?,
                identifier: reference.identifier
            }),
            "duplicate reported reference"
        );
    }
    anyhow::ensure!(
        row.references_state == 2 || references.is_empty(),
        "unreported reference set has stored values"
    );
    let references = if row.references_state == 2 {
        Some(references)
    } else {
        None
    };
    Ok(StationReport {
        name: reported(row.name_state, row.name)?,
        position: reported(
            row.position_state,
            position(PositionFields {
                kind: row.position_kind,
                locator: row.locator,
                latitude: row.latitude,
                longitude: row.longitude,
                length: row.grid_length,
            })?,
        )?,
        dxcc: reported(row.dxcc_state, row.dxcc)?,
        primary_subdivision: reported(row.primary_state, row.primary_subdivision)?,
        secondary_subdivision: reported(row.secondary_state, row.secondary_subdivision)?,
        cq_zone: reported(
            row.cq_state,
            row.cq_zone
                .map(|value| Ok::<_, anyhow::Error>(CqZone::new(value.try_into()?)?))
                .transpose()?,
        )?,
        itu_zone: reported(
            row.itu_state,
            row.itu_zone
                .map(|value| Ok::<_, anyhow::Error>(ItuZone::new(value.try_into()?)?))
                .transpose()?,
        )?,
        references: reported(row.references_state, references)?,
    })
}
