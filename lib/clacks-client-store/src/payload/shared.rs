//! Relational shared tags, reusable profiles, equipment and output profiles.

use super::contact;
use clacks_model::{
    export::{OutputProfile, OutputPurpose},
    tag::{AutomaticUploadPolicies, AutomaticUploadPolicy, LogbookTag},
};
use clacks_sync::record::{RecordId, RecordKey, RecordValue};
use sqlx::SqliteConnection;

/// Encode an automatic-upload policy without depending on display text.
fn policy(value: AutomaticUploadPolicy) -> &'static str {
    match value {
        AutomaticUploadPolicy::Enable => "enable",
        AutomaticUploadPolicy::Forbid => "forbid",
        AutomaticUploadPolicy::Neither => "neither",
    }
}

/// Reject unknown stored policy discriminators.
fn read_policy(value: &str) -> anyhow::Result<AutomaticUploadPolicy> {
    match value {
        "enable" => Ok(AutomaticUploadPolicy::Enable),
        "forbid" => Ok(AutomaticUploadPolicy::Forbid),
        "neither" => Ok(AutomaticUploadPolicy::Neither),
        _ => anyhow::bail!("unknown automatic upload policy"),
    }
}

/// Store a shared native root with owned snapshots and structural references.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
    value: &RecordValue,
) -> anyhow::Result<()> {
    let account = key.account.to_string();
    let id = key.id.to_string();
    match value {
        RecordValue::Tag(tag) => {
            let lotw = policy(tag.automatic_upload.lotw);
            let qrz = policy(tag.automatic_upload.qrz);
            sqlx::query_file!(
                "sql/tag/insert.sql",
                payload,
                account,
                id,
                tag.name,
                lotw,
                qrz
            )
            .execute(connection)
            .await?;
        }
        RecordValue::Profile(profile) => {
            let station =
                contact::insert_station(connection, key.account, profile.station()).await?;
            if let Some(equipment) = profile.default_equipment() {
                super::identity(
                    connection,
                    RecordKey {
                        account: key.account,
                        id: RecordId::Equipment(equipment),
                    },
                )
                .await?;
            }
            let equipment = profile.default_equipment().map(|id| id.to_string());
            sqlx::query_file!(
                "sql/profile/insert.sql",
                payload,
                account,
                id,
                profile.name(),
                station,
                equipment
            )
            .execute(connection)
            .await?;
        }
        RecordValue::Equipment(equipment) => {
            let snapshot =
                contact::insert_equipment(connection, key.account, equipment.values()).await?;
            sqlx::query_file!(
                "sql/equipment/configuration_insert.sql",
                payload,
                account,
                id,
                equipment.name(),
                snapshot
            )
            .execute(connection)
            .await?;
        }
        RecordValue::OutputProfile(profile) => {
            let purpose = match profile.purpose {
                OutputPurpose::OrdinaryInterchange => "ordinary",
                OutputPurpose::Targeted => "targeted",
            };
            let fields = serde_json::to_string(&profile.contextual_fields)?;
            sqlx::query_file!(
                "sql/export/profile_insert.sql",
                payload,
                account,
                id,
                profile.name,
                purpose,
                fields
            )
            .execute(&mut *connection)
            .await?;
        }
        _ => anyhow::bail!("not a shared snapshot payload"),
    }
    Ok(())
}

/// Restore a qualified shared root without fetching live profile or equipment defaults.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<RecordValue> {
    let account = key.account.to_string();
    let id_text = key.id.to_string();
    Ok(match key.id {
        RecordId::Tag(id) => {
            let row = sqlx::query_file!("sql/tag/select.sql", payload, account, id_text)
                .fetch_one(connection)
                .await?;
            RecordValue::Tag(Box::new(LogbookTag {
                id,
                name: row.name,
                automatic_upload: AutomaticUploadPolicies {
                    lotw: read_policy(&row.lotw_policy)?,
                    qrz: read_policy(&row.qrz_policy)?,
                },
            }))
        }
        RecordId::Profile(id) => {
            let row = sqlx::query_file!("sql/profile/select.sql", payload, account, id_text)
                .fetch_one(&mut *connection)
                .await?;
            RecordValue::Profile(Box::new(
                (clacks_model::profile::SavedLocationProfileFields {
                    id,
                    name: row.name,
                    station: (contact::load_station(connection, key.account, row.station_id)
                        .await?),
                    default_equipment: row.equipment_id.map(|id| id.parse()).transpose()?,
                })
                .try_into()?,
            ))
        }
        RecordId::Equipment(id) => {
            let row = sqlx::query_file!(
                "sql/equipment/configuration_select.sql",
                payload,
                account,
                id_text
            )
            .fetch_one(&mut *connection)
            .await?;
            RecordValue::Equipment(Box::new(
                (clacks_model::profile::EquipmentConfigurationFields {
                    id,
                    name: row.name,
                    values: (contact::load_equipment(connection, key.account, row.snapshot_id)
                        .await?)
                        .into_fields(),
                })
                .try_into()?,
            ))
        }
        RecordId::OutputProfile(id) => {
            let row = sqlx::query_file!("sql/export/profile_select.sql", payload, account, id_text)
                .fetch_one(&mut *connection)
                .await?;
            let purpose = match row.purpose.as_str() {
                "ordinary" => OutputPurpose::OrdinaryInterchange,
                "targeted" => OutputPurpose::Targeted,
                _ => anyhow::bail!("unknown output purpose"),
            };
            let fields = serde_json::from_str(&row.contextual_fields)?;
            RecordValue::OutputProfile(Box::new(OutputProfile {
                id,
                name: row.name,
                purpose,
                contextual_fields: fields,
            }))
        }
        _ => anyhow::bail!("not a shared snapshot identity"),
    })
}
