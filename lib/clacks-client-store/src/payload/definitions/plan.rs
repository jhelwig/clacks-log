//! Relational output plans, field bindings, partitions and expansions.

use super::{expression, ordinal, read_side, side};
use crate::rows::definitions::{BindingFields, PartitionFields};
use clacks_model::{
    export::rules::{
        BindingCondition, ExpansionRule, FieldBinding, GroupExportPlan, PartitionRule,
    },
    ids::AccountId,
};
use sqlx::SqliteConnection;

/// Store ordered field, partition and expansion rules under one group output plan.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    profile: Option<String>,
    plan: &GroupExportPlan,
) -> anyhow::Result<()> {
    if let Some(profile) = &profile {
        crate::payload::identity(
            connection,
            clacks_sync::record::RecordKey {
                account,
                id: clacks_sync::record::RecordId::OutputProfile(profile.parse()?),
            },
        )
        .await?;
    }
    let account = account.to_string();
    let plan_id = sqlx::query_file!("sql/export/plan_insert.sql", payload, account, profile)
        .fetch_one(&mut *connection)
        .await?
        .plan_id;
    for (ordinal, field) in plan.fields.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let expression = expression::insert(connection, payload, &field.value).await?;
        let when = match &field.when {
            BindingCondition::Always => BindingFields {
                condition: "always",
                binding: None,
                side: None,
            },
            BindingCondition::BoundPresent(binding) => BindingFields {
                condition: "bound",
                binding: Some(binding.as_str()),
                side: None,
            },
            BindingCondition::AssociationPresent(value) => BindingFields {
                condition: "association",
                binding: None,
                side: Some(side(*value)),
            },
        };
        sqlx::query_file!(
            "sql/export/field_insert.sql",
            plan_id,
            payload,
            ordinal,
            field.field,
            expression,
            when.condition,
            when.binding,
            when.side
        )
        .execute(&mut *connection)
        .await?;
    }
    for (ordinal, partition) in plan.file_partition.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let fields = match partition {
            PartitionRule::ByValue(value) => PartitionFields {
                expression_id: Some(expression::insert(connection, payload, value).await?),
                side: None,
                binding: None,
            },
            PartitionRule::EachReference {
                side: value,
                binding,
            } => PartitionFields {
                expression_id: None,
                side: Some(side(*value)),
                binding: Some(binding.as_str()),
            },
        };
        sqlx::query_file!(
            "sql/export/partition_insert.sql",
            plan_id,
            payload,
            ordinal,
            fields.expression_id,
            fields.side,
            fields.binding
        )
        .execute(&mut *connection)
        .await?;
    }
    for (ordinal, expansion) in plan.entry_expansion.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let side = side(expansion.side);
        sqlx::query_file!(
            "sql/export/expansion_insert.sql",
            plan_id,
            ordinal,
            side,
            expansion.binding,
            expansion.keep_one_if_empty
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Restore a plan's ordered rule lists with strict variant checks.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    payload: i64,
    plan: i64,
) -> anyhow::Result<GroupExportPlan> {
    let mut fields = Vec::new();
    for row in sqlx::query_file!("sql/export/field_select.sql", plan, payload)
        .fetch_all(&mut *connection)
        .await?
    {
        ordinal(fields.len(), row.ordinal)?;
        let when = match (BindingFields {
            condition: row.condition.as_str(),
            binding: row.binding.as_deref(),
            side: row.side.as_deref(),
        }) {
            BindingFields {
                condition: "always",
                binding: None,
                side: None,
            } => BindingCondition::Always,
            BindingFields {
                condition: "bound",
                binding: Some(binding),
                side: None,
            } => BindingCondition::BoundPresent(binding.to_owned()),
            BindingFields {
                condition: "association",
                binding: None,
                side: Some(side),
            } => BindingCondition::AssociationPresent(read_side(side)?),
            _ => anyhow::bail!("invalid binding condition"),
        };
        fields.push(FieldBinding {
            field: row.field,
            value: expression::load(connection, payload, row.expression_id).await?,
            when,
        });
    }
    let mut file_partition = Vec::new();
    for row in sqlx::query_file!("sql/export/partition_select.sql", plan, payload)
        .fetch_all(&mut *connection)
        .await?
    {
        ordinal(file_partition.len(), row.ordinal)?;
        file_partition.push(
            match (PartitionFields {
                expression_id: row.expression_id,
                side: row.side.as_deref(),
                binding: row.binding.as_deref(),
            }) {
                PartitionFields {
                    expression_id: Some(expression),
                    side: None,
                    binding: None,
                } => {
                    PartitionRule::ByValue(expression::load(connection, payload, expression).await?)
                }
                PartitionFields {
                    expression_id: None,
                    side: Some(side),
                    binding: Some(binding),
                } => PartitionRule::EachReference {
                    side: read_side(side)?,
                    binding: binding.to_owned(),
                },
                _ => anyhow::bail!("invalid partition shape"),
            },
        );
    }
    let mut entry_expansion = Vec::new();
    for row in sqlx::query_file!("sql/export/expansion_select.sql", plan)
        .fetch_all(&mut *connection)
        .await?
    {
        ordinal(entry_expansion.len(), row.ordinal)?;
        entry_expansion.push(ExpansionRule {
            side: read_side(&row.side)?,
            binding: row.binding,
            keep_one_if_empty: row.keep_empty != 0,
        });
        anyhow::ensure!(matches!(row.keep_empty, 0 | 1), "invalid keep-empty value");
    }
    Ok(GroupExportPlan {
        fields,
        file_partition,
        entry_expansion,
    })
}
