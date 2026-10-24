//! Typed reference-group definitions and their relational child mappings.

mod expression;
mod plan;

use crate::rows::definitions::CatalogFields;
use anyhow::Context;
use clacks_model::{
    catalog::{CatalogBinding, MetadataDisplayBinding},
    export::rules::{GroupExportMappings, GroupExportPlan},
    group::{GroupDefinition, IdentifierDefinition, TextNormalization},
    ids::{AccountId, GroupId},
    reference::StationSide,
};
use sqlx::SqliteConnection;
use std::collections::BTreeMap;

/// Encode the station side used by reference rules.
fn side(value: StationSide) -> &'static str {
    match value {
        StationSide::Own => "own",
        StationSide::Contacted => "contacted",
    }
}

/// Decode a required station side without guessing unknown discriminators.
fn read_side(value: &str) -> anyhow::Result<StationSide> {
    match value {
        "own" => Ok(StationSide::Own),
        "contacted" => Ok(StationSide::Contacted),
        _ => anyhow::bail!("unknown reference side"),
    }
}

/// Check that stored sequence ordinals are complete and contiguous.
fn ordinal(expected: usize, actual: i64) -> anyhow::Result<()> {
    anyhow::ensure!(
        i64::try_from(expected)? == actual,
        "noncontiguous definition sequence"
    );
    Ok(())
}

/// Store one validated data-defined group without rewriting recorded associations.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    group: &GroupDefinition,
) -> anyhow::Result<()> {
    let catalog = match &group.catalog {
        None => CatalogFields {
            catalog_id: None,
            catalog_display: None,
            catalog_lookup: None,
        },
        Some(binding) => CatalogFields {
            catalog_id: Some(binding.dataset_id.to_string()),
            catalog_display: Some(binding.display_name.clone()),
            catalog_lookup: Some(
                expression::insert(connection, payload, &binding.lookup_key).await?,
            ),
        },
    };
    let account_text = account.to_string();
    let id = group.id.to_string();
    sqlx::query_file!(
        "sql/group/insert.sql",
        payload,
        account_text,
        id,
        group.name,
        group.identifier.required,
        group.identifier.syntax,
        group.bears_location_context,
        catalog.catalog_id,
        catalog.catalog_display,
        catalog.catalog_lookup
    )
    .execute(&mut *connection)
    .await?;
    for (ordinal, rule) in group.identifier.normalization.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let rule = match rule {
            TextNormalization::Trim => "trim",
            TextNormalization::Uppercase => "uppercase",
            TextNormalization::Lowercase => "lowercase",
        };
        sqlx::query_file!("sql/group/normalization_insert.sql", payload, ordinal, rule)
            .execute(&mut *connection)
            .await?;
    }
    if let Some(binding) = &group.catalog {
        for (ordinal, detail) in binding.details.iter().enumerate() {
            let ordinal = i64::try_from(ordinal)?;
            sqlx::query_file!(
                "sql/group/detail_insert.sql",
                payload,
                ordinal,
                detail.label,
                detail.field
            )
            .execute(&mut *connection)
            .await?;
        }
    }
    plan::insert(
        connection,
        account,
        payload,
        None,
        &GroupExportPlan {
            fields: group.export_mappings.ordinary_interchange.clone(),
            ..Default::default()
        },
    )
    .await?;
    for (profile, plan) in &group.export_mappings.targeted {
        plan::insert(
            connection,
            account,
            payload,
            Some(profile.to_string()),
            plan,
        )
        .await?;
    }
    Ok(())
}

/// Restore a group's complete relational definition with group-specific error context.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    id: GroupId,
) -> anyhow::Result<GroupDefinition> {
    let account = account.to_string();
    let id_text = id.to_string();
    let row = sqlx::query_file!("sql/group/select.sql", payload, account, id_text)
        .fetch_one(&mut *connection)
        .await?;
    let mut normalization = Vec::new();
    for row in sqlx::query_file!("sql/group/normalization_select.sql", payload)
        .fetch_all(&mut *connection)
        .await?
    {
        ordinal(normalization.len(), row.ordinal)?;
        normalization.push(match row.transformation.as_str() {
            "trim" => TextNormalization::Trim,
            "uppercase" => TextNormalization::Uppercase,
            "lowercase" => TextNormalization::Lowercase,
            _ => anyhow::bail!("unknown normalization"),
        });
    }
    let mut details = Vec::new();
    for row in sqlx::query_file!("sql/group/detail_select.sql", payload)
        .fetch_all(&mut *connection)
        .await?
    {
        ordinal(details.len(), row.ordinal)?;
        details.push(MetadataDisplayBinding {
            label: row.label,
            field: row.field,
        });
    }
    let catalog = match (CatalogFields {
        catalog_id: row.catalog_id,
        catalog_display: row.catalog_display,
        catalog_lookup: row.catalog_lookup,
    }) {
        CatalogFields {
            catalog_id: None,
            catalog_display: None,
            catalog_lookup: None,
        } if details.is_empty() => None,
        CatalogFields {
            catalog_id: Some(dataset),
            catalog_display: Some(display_name),
            catalog_lookup: Some(expression),
        } => Some(CatalogBinding {
            dataset_id: dataset.parse()?,
            lookup_key: expression::load(connection, payload, expression)
                .await
                .with_context(|| format!("group {id} catalog.lookup_key"))?,
            display_name,
            details,
        }),
        _ => anyhow::bail!("group {id} has inconsistent catalog binding"),
    };
    let mut ordinary = None;
    let mut targeted = BTreeMap::new();
    for plan in sqlx::query_file!("sql/export/plan_select.sql", payload)
        .fetch_all(&mut *connection)
        .await?
    {
        let value = plan::load(connection, payload, plan.plan_id)
            .await
            .with_context(|| format!("group {id} export plan {}", plan.plan_id))?;
        if let Some(profile) = plan.profile_id {
            anyhow::ensure!(
                targeted.insert(profile.parse()?, value).is_none(),
                "group {id} duplicate output profile"
            );
        } else {
            anyhow::ensure!(
                ordinary.is_none()
                    && value.file_partition.is_empty()
                    && value.entry_expansion.is_empty(),
                "group {id} invalid ordinary plan"
            );
            ordinary = Some(value.fields);
        }
    }
    anyhow::ensure!(
        matches!(row.identifier_required, 0 | 1) && matches!(row.bears_location, 0 | 1),
        "group {id} invalid flag"
    );
    let group = GroupDefinition {
        id,
        name: row.name,
        identifier: IdentifierDefinition {
            required: row.identifier_required != 0,
            normalization,
            syntax: row.syntax,
        },
        bears_location_context: row.bears_location != 0,
        catalog,
        export_mappings: GroupExportMappings {
            ordinary_interchange: ordinary.context("ordinary plan is missing")?,
            targeted,
        },
    };
    let orphans = sqlx::query_file!("sql/export/orphan_expressions.sql", payload)
        .fetch_one(&mut *connection)
        .await?
        .orphans;
    anyhow::ensure!(orphans == 0, "group {id} contains unreferenced expressions");
    Ok(group)
}

#[cfg(test)]
mod tests;
