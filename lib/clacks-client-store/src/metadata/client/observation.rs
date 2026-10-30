//! Accepted envelopes and shared observations for client reconciliation.

use crate::{
    metadata::{clock, lineage},
    rows::client::ObservationRow,
};
use clacks_sync::{
    error::UnknownReason,
    record::{AcceptedRecord, Observation, RecordKey},
};
use sqlx::SqliteConnection;

/// Encode unavailable comparison evidence independently from native record state.
pub(crate) fn unknown(value: UnknownReason) -> &'static str {
    match value {
        UnknownReason::MissingBaseline => "baseline",
        UnknownReason::MissingLineage => "lineage",
        UnknownReason::MissingOriginal => "original",
        UnknownReason::CollectedContext => "collected",
    }
}

/// Decode unavailable evidence without equating it with certified absence.
pub(crate) fn read_unknown(value: &str) -> anyhow::Result<UnknownReason> {
    match value {
        "baseline" => Ok(UnknownReason::MissingBaseline),
        "lineage" => Ok(UnknownReason::MissingLineage),
        "original" => Ok(UnknownReason::MissingOriginal),
        "collected" => Ok(UnknownReason::CollectedContext),
        _ => anyhow::bail!("unknown missing-evidence discriminator"),
    }
}

/// Store an accepted envelope without modifying its author, payload or acceptance context.
pub(crate) async fn insert_envelope(
    connection: &mut SqliteConnection,
    value: &AcceptedRecord,
) -> anyhow::Result<i64> {
    let account = value.key().account.to_string();
    let kind = crate::payload::kind(value.key().id);
    let id = value.key().id.to_string();
    let timeline = value.stamp.accepted_at().timeline.to_string();
    let revision = i64::try_from(value.stamp.accepted_at().revision)?;
    let baseline = value.stamp.baseline().to_string();
    for candidate in sqlx::query_file!(
        "sql/client/envelope_candidates.sql",
        account,
        kind,
        id,
        timeline,
        revision,
        baseline
    )
    .fetch_all(&mut *connection)
    .await?
    {
        if load_envelope(connection, value.key(), candidate.envelope_id).await? == *value {
            return Ok(candidate.envelope_id);
        }
    }
    let payload = crate::payload::insert(connection, value.key(), value.state()).await?;
    let stamp = clock::insert_stamp(connection, value.key(), &value.stamp).await?;
    Ok(sqlx::query_file!(
        "sql/client/envelope_insert.sql",
        account,
        kind,
        id,
        payload,
        stamp
    )
    .fetch_one(connection)
    .await?
    .envelope_id)
}

/// Restore an envelope only under its complete native record key.
pub(crate) async fn load_envelope(
    connection: &mut SqliteConnection,
    key: RecordKey,
    envelope: i64,
) -> anyhow::Result<AcceptedRecord> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file!(
        "sql/client/envelope_select.sql",
        envelope,
        account,
        kind,
        id
    )
    .fetch_one(&mut *connection)
    .await?;
    AcceptedRecord::new(
        key,
        crate::payload::load(connection, key, row.payload_id).await?,
        clock::load_stamp(connection, key, row.stamp_id).await?,
    )
}

/// Store accepted, absent or unavailable shared knowledge as distinct typed variants.
pub(crate) async fn insert_observation(
    connection: &mut SqliteConnection,
    value: &Observation,
) -> anyhow::Result<i64> {
    value.validate()?;
    let key = value.key();
    crate::payload::identity(connection, key).await?;
    let row = match value {
        Observation::Accepted(value) => ObservationRow {
            variant: "accepted".into(),
            envelope_id: Some(insert_envelope(connection, value).await?),
            checkpoint_id: None,
            unknown_reason: None,
        },
        Observation::Absent { checkpoint, .. } => ObservationRow {
            variant: "absent".into(),
            envelope_id: None,
            checkpoint_id: Some(lineage::insert_checkpoint(connection, checkpoint).await?),
            unknown_reason: None,
        },
        Observation::Unavailable { reason, .. } => ObservationRow {
            variant: "unavailable".into(),
            envelope_id: None,
            checkpoint_id: None,
            unknown_reason: Some(unknown(*reason).into()),
        },
    };
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    Ok(sqlx::query_file!(
        "sql/client/observation_insert.sql",
        account,
        kind,
        id,
        row.variant,
        row.envelope_id,
        row.checkpoint_id,
        row.unknown_reason
    )
    .fetch_one(connection)
    .await?
    .observation_id)
}

/// Restore shared knowledge without treating a missing local row as an absence certificate.
pub(crate) async fn load_observation(
    connection: &mut SqliteConnection,
    key: RecordKey,
    observation: i64,
) -> anyhow::Result<Observation> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file_as!(
        ObservationRow,
        "sql/client/observation_select.sql",
        observation,
        account,
        kind,
        id
    )
    .fetch_one(&mut *connection)
    .await?;
    let value = match row {
        ObservationRow {
            variant,
            envelope_id: Some(envelope),
            checkpoint_id: None,
            unknown_reason: None,
        } if variant == "accepted" => {
            Observation::Accepted(load_envelope(connection, key, envelope).await?)
        }
        ObservationRow {
            variant,
            envelope_id: None,
            checkpoint_id: Some(checkpoint),
            unknown_reason: None,
        } if variant == "absent" => Observation::Absent {
            key,
            checkpoint: lineage::load_checkpoint(connection, key.account, checkpoint).await?,
        },
        ObservationRow {
            variant,
            envelope_id: None,
            checkpoint_id: None,
            unknown_reason: Some(reason),
        } if variant == "unavailable" => Observation::Unavailable {
            key,
            reason: read_unknown(&reason)?,
        },
        _ => anyhow::bail!("invalid shared observation variant fields"),
    };
    value.validate()?;
    Ok(value)
}
