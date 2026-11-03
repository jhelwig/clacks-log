//! Relational directory winners, separate from recorded station snapshots.

use crate::rows::{
    contact::PositionFields,
    directory::{ReferenceRow, SnapshotRow},
};
use clacks_model::{
    directory::{
        record::DirectoryRecord,
        values::{DirectoryFacts, DirectorySnapshot, DirectorySnapshotFields},
    },
    ids::{AccountId, QsoId},
    location::{CqZone, ItuZone, RecordedLocationFields},
    position::{Coordinates, Locator},
    reference::ReferenceAssociation,
    scalar::{RecordedCallsign, UtcTimestamp},
};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Persist a complete immutable directory payload with account-qualified child ownership.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    value: &DirectoryRecord,
) -> anyhow::Result<()> {
    super::identity(
        connection,
        RecordKey {
            account,
            id: RecordId::Qso(value.qso()),
        },
    )
    .await?;
    let owner = account.to_string();
    let qso = value.qso().to_string();
    sqlx::query_file!("sql/directory/root_insert.sql", payload, owner, qso)
        .execute(&mut *connection)
        .await?;
    for snapshot in value.snapshots() {
        let facts = snapshot.facts();
        let position = PositionFields::from(
            facts
                .location
                .as_ref()
                .and_then(|value| value.position().as_ref()),
        );
        let location = facts
            .location
            .as_ref()
            .map(|value| value.clone().into_fields())
            .unwrap_or_default();
        let key = snapshot.key().as_str();
        let provider = snapshot.provider().as_str();
        let seconds = snapshot.lookup_time().unix_seconds();
        let nanoseconds = i64::from(snapshot.lookup_time().subsecond_nanoseconds());
        let lookup_id = snapshot.lookup_id().to_string();
        let cq = location.cq_zone.map(|value| i64::from(value.get()));
        let itu = location.itu_zone.map(|value| i64::from(value.get()));
        let returned = facts.returned_callsign.as_ref().map(|value| value.as_str());
        let grid = facts.supplied_locator.as_ref().map(ToString::to_string);
        let latitude = facts
            .supplied_coordinates
            .map(|value| i64::from(value.latitude_microdegrees()));
        let longitude = facts
            .supplied_coordinates
            .map(|value| i64::from(value.longitude_microdegrees()));
        sqlx::query_file!(
            "sql/directory/snapshot_insert.sql",
            payload,
            owner,
            key,
            provider,
            seconds,
            nanoseconds,
            lookup_id,
            facts.name,
            position.kind,
            position.locator,
            position.latitude,
            position.longitude,
            position.length,
            location.dxcc,
            location.primary_subdivision,
            location.secondary_subdivision,
            cq,
            itu,
            facts.locality,
            returned,
            grid,
            latitude,
            longitude,
            facts.coordinate_source
        )
        .execute(&mut *connection)
        .await?;
        for reference in &facts.references {
            super::identity(
                connection,
                RecordKey {
                    account,
                    id: RecordId::Group(reference.group_id),
                },
            )
            .await?;
            let group = reference.group_id.to_string();
            let identifier = reference.identifier().as_deref();
            sqlx::query_file!(
                "sql/directory/reference_insert.sql",
                payload,
                owner,
                key,
                group,
                identifier
            )
            .execute(&mut *connection)
            .await?;
        }
    }
    Ok(())
}

/// Restore the exact retained lookup tuples and validate native optional fact shapes.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    qso: QsoId,
) -> anyhow::Result<DirectoryRecord> {
    let owner = account.to_string();
    let root = sqlx::query_file!("sql/directory/root_select.sql", payload, owner)
        .fetch_one(&mut *connection)
        .await?;
    anyhow::ensure!(
        root.record_id == qso.to_string(),
        "Directory payload parent differs from its identity"
    );
    let rows = sqlx::query_file_as!(
        SnapshotRow,
        "sql/directory/snapshot_select.sql",
        payload,
        owner
    )
    .fetch_all(&mut *connection)
    .await?;
    let mut value = DirectoryRecord::new(qso);
    for row in rows {
        let references = sqlx::query_file_as!(
            ReferenceRow,
            "sql/directory/reference_select.sql",
            payload,
            owner,
            row.lookup_key
        )
        .fetch_all(&mut *connection)
        .await?
        .into_iter()
        .map(|reference| {
            ReferenceAssociation::new(reference.group_id.parse()?, reference.identifier)
        })
        .collect::<anyhow::Result<_>>()?;
        value.record(snapshot(row, references)?)?;
    }
    Ok(value)
}

/// Decode one checked row without canonicalizing restored lookup keys as new requests.
fn snapshot(
    row: SnapshotRow,
    references: std::collections::BTreeSet<ReferenceAssociation>,
) -> anyhow::Result<DirectorySnapshot> {
    let position = PositionFields {
        kind: row.position_kind,
        locator: row.locator,
        latitude: row.latitude,
        longitude: row.longitude,
        length: row.grid_length,
    }
    .decode()?;
    let cq_zone = row
        .cq_zone
        .map(|value| Ok::<_, anyhow::Error>(CqZone::new(value.try_into()?)?))
        .transpose()?;
    let itu_zone = row
        .itu_zone
        .map(|value| Ok::<_, anyhow::Error>(ItuZone::new(value.try_into()?)?))
        .transpose()?;
    let has_location = position.is_some()
        || row.dxcc.is_some()
        || row.primary_subdivision.is_some()
        || row.secondary_subdivision.is_some()
        || cq_zone.is_some()
        || itu_zone.is_some();
    let location = if has_location {
        Some(
            RecordedLocationFields {
                position,
                dxcc: row.dxcc,
                primary_subdivision: row.primary_subdivision,
                secondary_subdivision: row.secondary_subdivision,
                cq_zone,
                itu_zone,
            }
            .try_into()?,
        )
    } else {
        None
    };
    let supplied_coordinates = match (row.supplied_latitude, row.supplied_longitude) {
        (None, None) => None,
        (Some(latitude), Some(longitude)) => Some(Coordinates::from_microdegrees(
            latitude.try_into()?,
            longitude.try_into()?,
        )?),
        _ => anyhow::bail!("Directory supplied coordinates are incomplete"),
    };
    DirectorySnapshot::new(DirectorySnapshotFields {
        provider: row.provider.parse()?,
        key: row.lookup_key.try_into()?,
        lookup_time: UtcTimestamp::new(row.lookup_seconds, row.lookup_nanoseconds.try_into()?)?,
        lookup_id: row.lookup_id.parse()?,
        facts: DirectoryFacts {
            name: row.name,
            location,
            references,
            locality: row.locality,
            returned_callsign: row
                .returned_callsign
                .as_deref()
                .map(RecordedCallsign::new)
                .transpose()?,
            supplied_locator: row
                .supplied_locator
                .as_deref()
                .map(Locator::parse)
                .transpose()?,
            supplied_coordinates,
            coordinate_source: row.coordinate_source,
        },
    })
}
