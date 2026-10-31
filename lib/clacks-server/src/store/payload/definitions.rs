//! Complete group configuration documents with relational output-profile links.

use clacks_model::{
    export::rules::GroupExportMappings,
    group::{GroupDefinition, GroupDefinitionFields},
    ids::{AccountId, GroupId},
};
use sqlx::PgConnection;
use std::collections::BTreeMap;

/// Store a checked group and its account-qualified output-profile relationships.
pub(super) async fn insert(
    connection: &mut PgConnection,
    account: AccountId,
    payload: i64,
    group: &GroupDefinition,
) -> anyhow::Result<()> {
    let identifier = serde_json::to_string(group.identifier())?;
    let catalog = group
        .catalog()
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let ordinary = serde_json::to_string(&group.export_mappings().ordinary_interchange)?;
    let account_text = account.to_string();
    let id = group.id().to_string();
    sqlx::query_file!(
        "sql/persistence/group/insert.sql",
        payload,
        account_text,
        id,
        group.name(),
        identifier,
        group.bears_location_context(),
        catalog,
        ordinary
    )
    .execute(&mut *connection)
    .await?;

    for (profile, plan) in &group.export_mappings().targeted {
        super::identity(
            connection,
            clacks_sync::record::RecordKey {
                account,
                id: clacks_sync::record::RecordId::OutputProfile(*profile),
            },
        )
        .await?;
        let profile = profile.to_string();
        let body = serde_json::to_string(plan)?;
        sqlx::query_file!(
            "sql/persistence/export/plan_insert.sql",
            payload,
            account_text,
            profile,
            body
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Restore checked configuration without normalizing historical associations.
pub(super) async fn load(
    connection: &mut PgConnection,
    account: AccountId,
    payload: i64,
    id: GroupId,
) -> anyhow::Result<GroupDefinition> {
    let account = account.to_string();
    let id_text = id.to_string();
    let row = sqlx::query_file!(
        "sql/persistence/group/select.sql",
        payload,
        account,
        id_text
    )
    .fetch_one(&mut *connection)
    .await?;
    let mut targeted = BTreeMap::new();
    for plan in sqlx::query_file!("sql/persistence/export/plan_select.sql", payload)
        .fetch_all(&mut *connection)
        .await?
    {
        targeted.insert(plan.profile_id.parse()?, serde_json::from_str(&plan.body)?);
    }
    GroupDefinitionFields {
        id,
        name: row.name,
        identifier: serde_json::from_str(&row.identifier_config)?,
        bears_location_context: row.bears_location,
        catalog: row
            .catalog_binding
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?,
        export_mappings: GroupExportMappings {
            ordinary_interchange: serde_json::from_str(&row.ordinary_bindings)?,
            targeted,
        },
    }
    .try_into()
    .map_err(Into::into)
}
