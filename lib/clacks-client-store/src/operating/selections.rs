//! Independent immutable source baselines for operating settings.

use crate::{payload, rows::operating::OperatingStateRow};
use clacks_model::ids::AccountId;
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use sqlx::SqliteConnection;

/// Restore selected sources from their exact copied payloads, not current shared values.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    account: AccountId,
    row: &OperatingStateRow,
    state: &mut super::OperatingState,
) -> anyhow::Result<()> {
    match (row.loaded_profile, row.profile_record.as_deref()) {
        (Some(payload), Some(id)) => {
            let key = RecordKey {
                account,
                id: RecordId::Profile(id.parse()?),
            };
            let RecordState::Live(RecordValue::Profile(value)) =
                payload::load(connection, key, payload).await?
            else {
                anyhow::bail!("selected profile baseline is not a live profile")
            };
            state.selected_profile = Some(*value);
        }
        (None, None) => {}
        _ => anyhow::bail!("incomplete selected profile baseline"),
    }
    match (row.loaded_equipment, row.equipment_record.as_deref()) {
        (Some(payload), Some(id)) => {
            let key = RecordKey {
                account,
                id: RecordId::Equipment(id.parse()?),
            };
            let RecordState::Live(RecordValue::Equipment(value)) =
                payload::load(connection, key, payload).await?
            else {
                anyhow::bail!("selected equipment baseline is not a live configuration")
            };
            state.selected_equipment = Some(*value);
        }
        (None, None) => {}
        _ => anyhow::bail!("incomplete selected equipment baseline"),
    }
    Ok(())
}

/// Retain exact source values as local payload roots without creating shared work.
pub(super) async fn store(
    connection: &mut SqliteConnection,
    account: AccountId,
    state: &super::OperatingState,
    row: &mut OperatingStateRow,
) -> anyhow::Result<()> {
    if let Some(profile) = &state.selected_profile {
        let key = RecordKey {
            account,
            id: RecordId::Profile(profile.id),
        };
        row.loaded_profile = Some(
            payload::insert(
                connection,
                key,
                &RecordState::Live(RecordValue::Profile(Box::new(profile.clone()))),
            )
            .await?,
        );
        row.profile_record = Some(profile.id.to_string());
    }
    if let Some(equipment) = &state.selected_equipment {
        let key = RecordKey {
            account,
            id: RecordId::Equipment(equipment.id),
        };
        row.loaded_equipment = Some(
            payload::insert(
                connection,
                key,
                &RecordState::Live(RecordValue::Equipment(Box::new(equipment.clone()))),
            )
            .await?,
        );
        row.equipment_record = Some(equipment.id.to_string());
    }
    Ok(())
}
