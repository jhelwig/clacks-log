//! Immutable relational values shared by accepted and unpublished roles.

pub(crate) mod contact;
mod definitions;
mod service;
mod shared;

use clacks_model::service::evidence::ServiceRecordId;
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use sqlx::SqliteConnection;

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
pub(crate) async fn identity(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<()> {
    let account = key.account.to_string();
    let discriminator = kind(key.id);
    let id = key.id.to_string();
    sqlx::query_file!("sql/payload/identity.sql", account, discriminator, id)
        .execute(connection)
        .await?;
    Ok(())
}

/// Store one validated immutable payload in the caller's transaction.
pub(crate) async fn insert(
    connection: &mut SqliteConnection,
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
        "sql/payload/insert.sql",
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
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<RecordState> {
    let row = sqlx::query_file!("sql/payload/select.sql", payload)
        .fetch_one(&mut *connection)
        .await?;
    anyhow::ensure!(
        row.account == key.account.to_string()
            && row.kind == kind(key.id)
            && row.record_id == key.id.to_string(),
        "payload identity differs from requested record"
    );
    let roots = sqlx::query_file!("sql/payload/root_count.sql", payload)
        .fetch_one(&mut *connection)
        .await?
        .roots;
    let state = if row.deleted != 0 {
        anyhow::ensure!(row.deleted == 1, "invalid payload deletion discriminator");
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

#[cfg(test)]
mod tests {
    use crate::store::{ClientStore, StoreSettings};
    use crate::test_support as support;
    use clacks_model::{reference::ReferenceAssociation, scalar::Milliwatts};
    use support::id;

    #[tokio::test]
    async fn recorded_contacts_are_relationally_queryable_and_reject_incomplete_roots()
    -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let file = directory.path().join("workspace.sqlite");
        let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
        let mut qso = support::qso(id(10));
        qso.tags = [id(40), id(41)].into();
        qso.own_context.transmit_power = Some(Milliwatts::new(100_000)?);
        qso.own_station.references = [None, Some("ABC-1"), Some("ABC-2")]
            .into_iter()
            .map(|identifier| ReferenceAssociation {
                group_id: id(30),
                identifier: identifier.map(String::from),
            })
            .collect();
        let key = support::qso_key(qso.id);
        let mut tx = store.write().await?;
        let payload = super::insert(&mut tx, key, &support::contact(qso)).await?;
        let group = id::<clacks_model::ids::GroupId>(30).to_string();
        assert!(
            sqlx::query_file!("tests/sql/duplicate_group_only.sql", group, payload)
                .execute(&mut *tx)
                .await
                .is_err()
        );
        tx.commit().await?;
        store.close().await?;

        let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
        let mut tx = store.write().await?;
        let query = sqlx::query_file!("tests/sql/contact_query.sql", 14_000_000_i64, 100_000_i64)
            .fetch_all(&mut *tx)
            .await?;
        assert_eq!(query.len(), 1);
        let row = query.first().unwrap();
        assert_eq!(
            (row.frequency, row.power, row.tags, row.references),
            (Some(14_074_000), Some(100_000), 2, 3)
        );
        sqlx::query_file!("tests/sql/remove_contact_root.sql", payload)
            .execute(&mut *tx)
            .await?;
        assert!(super::load(&mut tx, key, payload).await.is_err());
        tx.rollback().await?;
        store.close().await?;
        Ok(())
    }
}
