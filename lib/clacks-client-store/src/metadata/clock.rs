//! Record-qualified vector and accepted-context storage shared by payload roles.

use clacks_sync::{
    clock::{ClockActor, RecordVector},
    record::{Position, RecordKey, VersionStamp},
};
use sqlx::SqliteConnection;

/// Store exact nonzero vector components under their native record identity.
pub(crate) async fn insert_vector(
    connection: &mut SqliteConnection,
    key: RecordKey,
    vector: &RecordVector,
) -> anyhow::Result<i64> {
    crate::payload::identity(connection, key).await?;
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file!("sql/vector/insert.sql", account, kind, id)
        .fetch_one(&mut *connection)
        .await?;
    for (actor, counter) in vector.entries() {
        let (kind, actor) = match actor {
            ClockActor::Client(id) => ("client", id.to_string()),
            ClockActor::Server(id) => ("server", id.to_string()),
        };
        let counter = i64::try_from(counter)?;
        sqlx::query_file!(
            "sql/vector/component_insert.sql",
            row.vector_id,
            kind,
            actor,
            counter
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(row.vector_id)
}

/// Restore an exact vector after checking its complete record scope.
pub(crate) async fn load_vector(
    connection: &mut SqliteConnection,
    key: RecordKey,
    vector: i64,
) -> anyhow::Result<RecordVector> {
    let owner = sqlx::query_file!("sql/vector/owner.sql", vector)
        .fetch_one(&mut *connection)
        .await?;
    anyhow::ensure!(
        owner.account == key.account.to_string()
            && owner.kind == crate::payload::kind(key.id)
            && owner.record_id == key.id.to_string(),
        "vector record scope mismatch"
    );
    let mut entries = Vec::new();
    for row in sqlx::query_file!("sql/vector/select.sql", vector)
        .fetch_all(connection)
        .await?
    {
        let actor = match row.actor_kind.as_str() {
            "client" => ClockActor::Client(row.actor_id.parse()?),
            "server" => ClockActor::Server(row.actor_id.parse()?),
            _ => anyhow::bail!("unknown clock actor role"),
        };
        anyhow::ensure!(row.counter > 0, "stored vector component must be positive");
        entries.push((actor, row.counter.try_into()?));
    }
    Ok(RecordVector::from_entries(entries)?)
}

/// Store accepted-context coordinates without inventing their baseline interpretation.
pub(crate) async fn insert_stamp(
    connection: &mut SqliteConnection,
    key: RecordKey,
    stamp: &VersionStamp,
) -> anyhow::Result<i64> {
    stamp.validate()?;
    let vector = insert_vector(connection, key, &stamp.vector).await?;
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let timeline = stamp.accepted_at.timeline.to_string();
    let revision = i64::try_from(stamp.accepted_at.revision)?;
    let baseline = stamp.baseline.to_string();
    Ok(sqlx::query_file!(
        "sql/stamp/insert.sql",
        account,
        kind,
        id,
        timeline,
        revision,
        baseline,
        vector
    )
    .fetch_one(connection)
    .await?
    .stamp_id)
}

/// Restore a record-qualified context without treating a baseline ID as installed evidence.
pub(crate) async fn load_stamp(
    connection: &mut SqliteConnection,
    key: RecordKey,
    stamp: i64,
) -> anyhow::Result<VersionStamp> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file!("sql/stamp/select.sql", stamp, account, kind, id)
        .fetch_one(&mut *connection)
        .await?;
    let stamp = VersionStamp {
        accepted_at: Position {
            timeline: row.timeline_id.parse()?,
            revision: row.revision.try_into()?,
        },
        baseline: row.baseline_id.parse()?,
        vector: load_vector(connection, key, row.vector_id).await?,
    };
    stamp.validate()?;
    Ok(stamp)
}
