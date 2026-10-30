//! Frozen proposals and their target and dependency conditions.

use crate::{
    metadata::{clock, lineage},
    rows::client::ConditionRow,
};
use clacks_model::ids::AccountId;
use clacks_sync::{
    client::{PreparedChange, VersionCondition},
    record::RecordKey,
};
use sqlx::SqliteConnection;

/// Store an actual target or reference compare condition.
pub(crate) async fn insert_condition(
    connection: &mut SqliteConnection,
    value: &VersionCondition,
) -> anyhow::Result<i64> {
    value.validate()?;
    let key = value.key();
    crate::payload::identity(connection, key).await?;
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id).to_string();
    let record_id = key.id.to_string();
    Ok(match value {
        VersionCondition::Is { stamp, .. } => {
            let stamp = clock::insert_stamp(connection, key, stamp).await?;
            sqlx::query_file!(
                "sql/condition/insert.sql",
                account,
                kind,
                record_id,
                "is",
                stamp,
                None::<i64>
            )
            .fetch_one(connection)
            .await?
            .condition_id
        }
        VersionCondition::Absent { checkpoint, .. } => {
            let checkpoint = lineage::insert_checkpoint(connection, checkpoint).await?;
            sqlx::query_file!(
                "sql/condition/insert.sql",
                account,
                kind,
                record_id,
                "absent",
                None::<i64>,
                checkpoint
            )
            .fetch_one(connection)
            .await?
            .condition_id
        }
    })
}

/// Restore a condition's own record identity, including differently keyed dependencies.
pub(crate) async fn load_condition(
    connection: &mut SqliteConnection,
    account: AccountId,
    condition: i64,
) -> anyhow::Result<VersionCondition> {
    let row = sqlx::query_file_as!(ConditionRow, "sql/condition/select.sql", condition)
        .fetch_one(&mut *connection)
        .await?;
    anyhow::ensure!(
        row.account == account.to_string(),
        "condition crosses account"
    );
    let key = RecordKey {
        account,
        id: crate::payload::record_id(&row.kind, &row.record_id)?,
    };
    let value = match row {
        ConditionRow {
            variant,
            stamp_id: Some(stamp),
            checkpoint_id: None,
            ..
        } if variant == "is" => VersionCondition::Is {
            key,
            stamp: clock::load_stamp(connection, key, stamp).await?,
        },
        ConditionRow {
            variant,
            stamp_id: None,
            checkpoint_id: Some(checkpoint),
            ..
        } if variant == "absent" => VersionCondition::Absent {
            key,
            checkpoint: lineage::load_checkpoint(connection, account, checkpoint).await?,
        },
        _ => anyhow::bail!("invalid condition variant fields"),
    };
    value.validate()?;
    Ok(value)
}

/// Store one frozen proposal independently of later working values.
pub(super) async fn insert_prepared(
    connection: &mut SqliteConnection,
    value: &PreparedChange,
) -> anyhow::Result<i64> {
    let key = value.key();
    let payload = crate::payload::insert(connection, key, value.state()).await?;
    let vector = clock::insert_vector(connection, key, value.vector()).await?;
    let condition = insert_condition(connection, value.expected()).await?;
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let actor = value.actor().to_string();
    let prepared = sqlx::query_file!(
        "sql/prepared/insert.sql",
        account,
        kind,
        id,
        payload,
        vector,
        actor,
        condition
    )
    .fetch_one(&mut *connection)
    .await?
    .prepared_id;
    for (ordinal, dependency) in value.dependencies().iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let condition = insert_condition(connection, dependency).await?;
        sqlx::query_file!(
            "sql/prepared/dependency_insert.sql",
            prepared,
            ordinal,
            condition
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(prepared)
}

/// Restore frozen values, their allocated vector and ordered conditional dependencies.
pub(super) async fn load_prepared(
    connection: &mut SqliteConnection,
    key: RecordKey,
    prepared: i64,
) -> anyhow::Result<PreparedChange> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file!("sql/prepared/select.sql", prepared, account, kind, id)
        .fetch_one(&mut *connection)
        .await?;
    let state = crate::payload::load(connection, key, row.payload_id).await?;
    let vector = clock::load_vector(connection, key, row.vector_id).await?;
    let expected = load_condition(connection, key.account, row.condition_id).await?;
    let mut dependencies = Vec::new();
    for row in sqlx::query_file!("sql/prepared/dependency_select.sql", prepared)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(dependencies.len())?,
            "noncontiguous prepared dependencies"
        );
        dependencies.push(load_condition(connection, key.account, row.condition_id).await?);
    }
    clacks_sync::client::PreparedChangeFields {
        key,
        state,
        vector,
        actor: row.actor_id.parse()?,
        expected,
        dependencies,
    }
    .try_into()
}
