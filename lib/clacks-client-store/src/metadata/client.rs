//! Exact accepted, working, original, frozen and retained client-state roles.

pub(crate) mod observation;
pub(crate) mod original;
pub(crate) mod prepared;

use clacks_sync::{
    client::{ClientRecord, LocalCounter, PendingRecord},
    record::RecordKey,
};
use observation::{insert_envelope, insert_observation, load_envelope, load_observation};
use original::{insert_original, insert_witness, load_original, load_witness};
use prepared::{insert_prepared, load_prepared};
use sqlx::SqliteConnection;

/// Store a pending role without replacing its original with newer shared values.
async fn insert_pending(
    connection: &mut SqliteConnection,
    value: &PendingRecord,
) -> anyhow::Result<i64> {
    let key = value.key;
    let original = insert_original(connection, &value.original).await?;
    let working = crate::payload::insert(connection, key, &value.working).await?;
    let prepared = match &value.prepared {
        Some(prepared) => Some(insert_prepared(connection, prepared).await?),
        None => None,
    };
    let witness = match &value.witness {
        Some(witness) => Some(insert_witness(connection, witness, original).await?),
        None => None,
    };
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    Ok(sqlx::query_file!(
        "sql/client/pending_insert.sql",
        account,
        kind,
        id,
        original,
        working,
        prepared,
        witness
    )
    .fetch_one(connection)
    .await?
    .pending_id)
}

/// Restore independently owned working, original and frozen values for one record.
async fn load_pending(
    connection: &mut SqliteConnection,
    key: RecordKey,
    pending: i64,
) -> anyhow::Result<PendingRecord> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file!("sql/client/pending_select.sql", pending, account, kind, id)
        .fetch_one(&mut *connection)
        .await?;
    let original = load_original(connection, key, row.original_id).await?;
    let working = crate::payload::load(connection, key, row.working_payload).await?;
    let prepared = match row.prepared_id {
        Some(prepared) => Some(load_prepared(connection, key, prepared).await?),
        None => None,
    };
    let witness = match row.witness_id {
        Some(witness) => Some(load_witness(connection, key, witness).await?),
        None => None,
    };
    Ok(PendingRecord {
        key,
        original,
        working,
        prepared,
        witness,
    })
}

/// Replace current client role references inside the caller's existing write transaction.
pub(crate) async fn store(
    connection: &mut SqliteConnection,
    value: &ClientRecord,
) -> anyhow::Result<()> {
    value.validate()?;
    if load(connection, value.key).await?.as_ref() == Some(value) {
        return Ok(());
    }
    let latest = insert_observation(connection, &value.latest).await?;
    let pending = match &value.pending {
        Some(pending) => Some(insert_pending(connection, pending).await?),
        None => None,
    };
    let account = value.key.account.to_string();
    let kind = crate::payload::kind(value.key.id);
    let id = value.key.id.to_string();
    sqlx::query_file!("sql/client/store.sql", account, kind, id, latest, pending)
        .execute(&mut *connection)
        .await?;
    sqlx::query_file!("sql/client/retained_clear.sql", account, kind, id)
        .execute(&mut *connection)
        .await?;
    for (ordinal, retained) in value.retained.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let envelope = insert_envelope(connection, retained).await?;
        sqlx::query_file!(
            "sql/client/retained_insert.sql",
            account,
            kind,
            id,
            ordinal,
            envelope
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Read a complete native client record without inferring shared absence for missing rows.
pub(crate) async fn load(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<Option<ClientRecord>> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let Some(row) = sqlx::query_file!("sql/client/select.sql", account, kind, id)
        .fetch_optional(&mut *connection)
        .await?
    else {
        return Ok(None);
    };
    let latest = load_observation(connection, key, row.latest_observation).await?;
    let pending = match row.pending_id {
        Some(pending) => Some(load_pending(connection, key, pending).await?),
        None => None,
    };
    let mut retained = Vec::new();
    for row in sqlx::query_file!("sql/client/retained_select.sql", account, kind, id)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(retained.len())?,
            "noncontiguous retained alternatives"
        );
        retained.push(load_envelope(connection, key, row.envelope_id).await?);
    }
    let value = ClientRecord {
        key,
        latest,
        pending,
        retained,
    };
    value.validate()?;
    Ok(Some(value))
}

/// Read one actor's durable reservation, using zero only before any reservation exists.
pub(crate) async fn counter(
    connection: &mut SqliteConnection,
    key: RecordKey,
    actor: clacks_model::ids::ClientActorId,
) -> anyhow::Result<LocalCounter> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let actor_text = actor.to_string();
    let row = sqlx::query_file!("sql/counter/select.sql", account, kind, id, actor_text)
        .fetch_optional(connection)
        .await?;
    Ok(LocalCounter {
        actor,
        high_water: match row {
            Some(row) => row.high_water.try_into()?,
            None => 0,
        },
    })
}

/// Save a nondecreasing actor reservation in the same transaction as its frozen proposal.
pub(crate) async fn store_counter(
    connection: &mut SqliteConnection,
    key: RecordKey,
    value: &LocalCounter,
) -> anyhow::Result<()> {
    crate::payload::identity(connection, key).await?;
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let actor = value.actor.to_string();
    let high_water = i64::try_from(value.high_water)?;
    anyhow::ensure!(
        sqlx::query_file!(
            "sql/counter/store.sql",
            account,
            kind,
            id,
            actor,
            high_water
        )
        .execute(connection)
        .await?
        .rows_affected()
            == 1,
        "client counter cannot regress"
    );
    Ok(())
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
