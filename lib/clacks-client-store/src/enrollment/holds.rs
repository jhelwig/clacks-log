//! Local admission evidence kept outside synchronized record values.

use crate::metadata::review;
use clacks_model::ids::ClientActorId;
use clacks_sync::{
    client::{ClientRecord, ClientUpdate},
    record::{Observation, RecordKey},
};
use sqlx::SqliteConnection;

/// Read a retained former actor before loading synchronization evidence or allocating counters.
pub(crate) async fn actor_in(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<Option<ClientActorId>> {
    let row = sqlx::query_file!(
        "sql/enrollment/hold.sql",
        key.account.to_string(),
        crate::payload::kind(key.id),
        key.id.to_string()
    )
    .fetch_optional(connection)
    .await?;
    row.and_then(|row| row.authorization_hold)
        .map(|value| value.parse().map_err(Into::into))
        .transpose()
}

/// Make an ordinary actor-exclusion review once real shared comparison state exists.
pub(crate) async fn review_in(
    connection: &mut SqliteConnection,
    record: &ClientRecord,
) -> anyhow::Result<Option<clacks_sync::client::ReviewInput>> {
    let Some(actor) = actor_in(connection, record.key()).await? else {
        return Ok(None);
    };
    if record.pending().is_none() || matches!(record.latest(), Observation::Unavailable { .. }) {
        return Ok(None);
    }
    let update = clacks_sync::client::receive_rejection(
        record,
        &clacks_sync::acceptance::RejectionReason::ActorUnknown { actor },
    )?;
    review::store(connection, record.key(), update.review.as_ref()).await?;
    Ok(update.review)
}

/// Preserve old original/frozen work while incorporating an actual incoming shared observation.
pub(crate) async fn preserve_in(
    connection: &mut SqliteConnection,
    previous: &ClientRecord,
    update: &mut ClientUpdate,
) -> anyhow::Result<()> {
    if actor_in(connection, previous.key()).await?.is_some() && previous.pending().is_some() {
        let mut fields = update.record.clone().into_fields();
        fields.pending = previous.pending().cloned();
        update.record = fields.try_into()?;
    }
    Ok(())
}

/// Release only the former actor whose exact review was explicitly resolved.
pub(crate) async fn clear_in(
    connection: &mut SqliteConnection,
    key: RecordKey,
    former: ClientActorId,
) -> anyhow::Result<()> {
    sqlx::query_file!(
        "sql/enrollment/clear_hold.sql",
        key.account.to_string(),
        crate::payload::kind(key.id),
        key.id.to_string(),
        former.to_string()
    )
    .execute(connection)
    .await?;
    Ok(())
}

/// Create held-work reviews after a completed receipt, including certified full-view absence.
pub(crate) async fn completed_in(
    connection: &mut SqliteConnection,
    checkpoint: &clacks_sync::account::InstalledCheckpoint,
    full: bool,
) -> anyhow::Result<()> {
    let keys = sqlx::query_file!(
        "sql/enrollment/held_records.sql",
        checkpoint.account.to_string()
    )
    .fetch_all(&mut *connection)
    .await?;
    for row in keys {
        let key = RecordKey {
            account: checkpoint.account,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        let mut record = crate::metadata::client::load(connection, key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("held record disappeared inside its transaction"))?;
        if full && !matches!(record.latest(), Observation::Accepted(_)) {
            let mut fields = record.into_fields();
            fields.latest = Observation::Absent {
                key,
                checkpoint: checkpoint.clone(),
            };
            record = fields.try_into()?;
            crate::metadata::client::store(connection, &record).await?;
        }
        review_in(connection, &record).await?;
    }
    Ok(())
}
