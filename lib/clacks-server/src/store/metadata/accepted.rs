//! Accepted envelopes retain native values and their original context.

use super::clock;
use clacks_sync::record::{AcceptedRecord, RecordKey};
use sqlx::PgConnection;

/// Store an accepted envelope without modifying its author, payload or acceptance context.
pub(crate) async fn insert_envelope(
    connection: &mut PgConnection,
    value: &AcceptedRecord,
) -> anyhow::Result<i64> {
    let account = value.key().account.to_string();
    let kind = crate::store::payload::kind(value.key().id);
    let id = value.key().id.to_string();
    let timeline = value.stamp.accepted_at().timeline.to_string();
    let revision = i64::try_from(value.stamp.accepted_at().revision)?;
    let baseline = value.stamp.baseline().to_string();
    for candidate in sqlx::query_file!(
        "sql/persistence/client/envelope_candidates.sql",
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
    let payload = crate::store::payload::insert(connection, value.key(), value.state()).await?;
    let stamp = clock::insert_stamp(connection, value.key(), &value.stamp).await?;
    Ok(sqlx::query_file!(
        "sql/persistence/client/envelope_insert.sql",
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
    connection: &mut PgConnection,
    key: RecordKey,
    envelope: i64,
) -> anyhow::Result<AcceptedRecord> {
    let account = key.account.to_string();
    let kind = crate::store::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file!(
        "sql/persistence/client/envelope_select.sql",
        envelope,
        account,
        kind,
        id
    )
    .fetch_one(&mut *connection)
    .await?;
    AcceptedRecord::new(
        key,
        crate::store::payload::load(connection, key, row.payload_id).await?,
        clock::load_stamp(connection, key, row.stamp_id).await?,
    )
}
