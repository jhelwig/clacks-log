//! Source-only imported upload and confirmation comparison baselines.

use super::{identity, projection, reference, report};
use clacks_model::{
    location::{CqZone, ItuZone, RecordedLocation},
    scalar::{RecordedCallsign, UtcSecond},
    service::evidence::{
        ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim, ServiceRecordId,
        submission::ConfirmationBasis,
    },
};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Store imported positive status and all source-only applicability evidence.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
    value: &ImportedAssertion,
) -> anyhow::Result<()> {
    reference(connection, key, RecordId::Qso(value.qso)).await?;
    if let Some(destination) = value.destination {
        reference(
            connection,
            key,
            RecordId::Service(ServiceRecordId::Destination(destination)),
        )
        .await?;
    }
    let fields =
        projection::fields(connection, payload, "imported", &value.baseline.fields).await?;
    let account = key.account.to_string();
    let qso = value.qso.to_string();
    let service = identity::service(value.service);
    let destination = value.destination.map(|id| id.to_string());
    let claim = match value.claim {
        ImportedClaim::Uploaded => "uploaded",
        ImportedClaim::Confirmed => "confirmed",
    };
    let date = value.date.map(UtcSecond::unix_seconds);
    let end = value.baseline.native_end.map(UtcSecond::unix_seconds);
    let has_location = value.baseline.own_location.is_some();
    sqlx::query_file!(
        "sql/service/assertion_insert.sql",
        payload,
        account,
        qso,
        service,
        destination,
        claim,
        date,
        value.provenance.source,
        value.provenance.record,
        value.provenance.mapping,
        end,
        fields,
        has_location
    )
    .execute(&mut *connection)
    .await?;
    for field in &value.baseline.coverage {
        sqlx::query_file!("sql/service/coverage_insert.sql", payload, field)
            .execute(&mut *connection)
            .await?;
    }
    let basis = &value.baseline.confirmation;
    let own = basis.own_call.as_ref().map(RecordedCallsign::as_str);
    let worked = basis.worked_call.as_ref().map(RecordedCallsign::as_str);
    let start = basis.start.map(UtcSecond::unix_seconds);
    let frequency = basis
        .frequency
        .map(|value| i64::try_from(value.get()))
        .transpose()?;
    let receive_frequency = basis
        .receive_frequency
        .map(|value| i64::try_from(value.get()))
        .transpose()?;
    let mode = basis.mode.as_ref().map(|mode| mode.concrete.as_str());
    let parent = basis.mode.as_ref().and_then(|mode| mode.parent.as_deref());
    let group = basis.mode.as_ref().and_then(|mode| mode.group.as_deref());
    sqlx::query_file!(
        "sql/service/basis_insert.sql",
        payload,
        basis.uninterpreted_band,
        basis.uninterpreted_receive_band,
        own,
        worked,
        start,
        basis.band,
        basis.receive_band,
        frequency,
        receive_frequency,
        mode,
        parent,
        group
    )
    .execute(&mut *connection)
    .await?;
    if let Some(location) = &value.baseline.own_location {
        let position = crate::rows::contact::PositionFields::new(location.position.as_ref());
        let cq = location.cq_zone.map(|value| i64::from(value.get()));
        let itu = location.itu_zone.map(|value| i64::from(value.get()));
        sqlx::query_file!(
            "sql/service/location_insert.sql",
            payload,
            position.kind,
            position.locator,
            position.latitude,
            position.longitude,
            position.length,
            location.dxcc,
            location.primary_subdivision,
            location.secondary_subdivision,
            cq,
            itu
        )
        .execute(connection)
        .await?;
    }
    Ok(())
}

/// Restore imported status without inventing a dispatch or enriching its source baseline.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<ImportedAssertion> {
    let RecordId::Service(ServiceRecordId::Assertion(id)) = key.id else {
        anyhow::bail!("not assertion identity")
    };
    let account = key.account.to_string();
    let row = sqlx::query_file!("sql/service/assertion_select.sql", payload, account)
        .fetch_one(&mut *connection)
        .await?;
    let service = identity::read_service(&row.service)?;
    let fields = projection::load_fields(connection, payload, "imported", row.fields_map).await?;
    let coverage = sqlx::query_file!("sql/service/coverage_select.sql", payload)
        .fetch_all(&mut *connection)
        .await?
        .into_iter()
        .map(|row| row.field)
        .collect();
    let basis = sqlx::query_file!("sql/service/basis_select.sql", payload)
        .fetch_one(&mut *connection)
        .await?;
    let confirmation = ConfirmationBasis {
        uninterpreted_band: basis.uninterpreted_band,
        uninterpreted_receive_band: basis.uninterpreted_receive_band,
        own_call: basis
            .own_call
            .map(|call| RecordedCallsign::new(&call))
            .transpose()?,
        worked_call: basis
            .worked_call
            .map(|call| RecordedCallsign::new(&call))
            .transpose()?,
        start: basis.start.map(UtcSecond::from_unix_seconds),
        band: basis.band,
        receive_band: basis.receive_band,
        frequency: report::hertz(basis.frequency)?,
        receive_frequency: report::hertz(basis.receive_frequency)?,
        mode: report::mode(basis.mode, basis.mode_parent, basis.mode_group)?,
    };
    let location = sqlx::query_file!("sql/service/location_select.sql", payload)
        .fetch_optional(connection)
        .await?;
    anyhow::ensure!(
        matches!(row.has_location, 0 | 1) && (row.has_location != 0) == location.is_some(),
        "imported location presence mismatch"
    );
    let own_location = location
        .map(|row| {
            Ok::<_, anyhow::Error>(RecordedLocation {
                position: report::position(crate::rows::contact::PositionFields {
                    kind: row.position_kind,
                    locator: row.locator,
                    latitude: row.latitude,
                    longitude: row.longitude,
                    length: row.grid_length,
                })?,
                dxcc: row.dxcc,
                primary_subdivision: row.primary_subdivision,
                secondary_subdivision: row.secondary_subdivision,
                cq_zone: row
                    .cq_zone
                    .map(|value| Ok::<_, anyhow::Error>(CqZone::new(value.try_into()?)?))
                    .transpose()?,
                itu_zone: row
                    .itu_zone
                    .map(|value| Ok::<_, anyhow::Error>(ItuZone::new(value.try_into()?)?))
                    .transpose()?,
            })
        })
        .transpose()?;
    Ok(ImportedAssertion {
        id,
        qso: row.qso_id.parse()?,
        service,
        destination: row.destination_id.map(|id| id.parse()).transpose()?,
        claim: match row.claim.as_str() {
            "uploaded" => ImportedClaim::Uploaded,
            "confirmed" => ImportedClaim::Confirmed,
            _ => anyhow::bail!("unknown imported positive claim"),
        },
        date: row.date.map(UtcSecond::from_unix_seconds),
        provenance: ImportProvenance {
            source: row.source,
            record: row.source_record,
            mapping: row.mapping,
        },
        baseline: ImportedBaseline {
            native_end: row.native_end.map(UtcSecond::from_unix_seconds),
            service,
            fields,
            own_location,
            confirmation,
            coverage,
        },
    })
}
