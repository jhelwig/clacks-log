//! Immutable relational values shared by accepted and unpublished roles.

pub(crate) mod contact;
mod definitions;
mod service;
mod shared;

use clacks_model::service::evidence::ServiceRecordId;
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use sqlx::PgConnection;

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
pub(crate) async fn identity(connection: &mut PgConnection, key: RecordKey) -> anyhow::Result<()> {
    let account = key.account.to_string();
    let discriminator = kind(key.id);
    let id = key.id.to_string();
    sqlx::query_file!(
        "sql/persistence/payload/identity.sql",
        account,
        discriminator,
        id
    )
    .execute(connection)
    .await?;
    Ok(())
}

/// Store one validated immutable payload in the caller's transaction.
pub(crate) async fn insert(
    connection: &mut PgConnection,
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
        "sql/persistence/payload/insert.sql",
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
    connection: &mut PgConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<RecordState> {
    let row = sqlx::query_file!(
        "sql/persistence/payload/select.sql",
        payload,
        key.account.to_string(),
        kind(key.id),
        key.id.to_string()
    )
    .fetch_one(&mut *connection)
    .await?;
    let roots = sqlx::query_file!("sql/persistence/payload/root_count.sql", payload)
        .fetch_one(&mut *connection)
        .await?
        .roots;
    let state = if row.deleted {
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
