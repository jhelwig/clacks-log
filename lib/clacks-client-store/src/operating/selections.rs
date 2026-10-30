//! Store profile and equipment baselines used to detect later saved-setting changes.

use crate::{payload, rows::operating::OperatingStateRow};
use clacks_model::ids::AccountId;
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use sqlx::SqliteConnection;

/// Restore selected sources from their exact copied payloads, not current shared values.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    account: AccountId,
    row: &OperatingStateRow,
) -> anyhow::Result<LoadedSources> {
    let mut sources = LoadedSources {
        profile: None,
        equipment: None,
    };
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
            sources.profile = Some(*value);
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
            sources.equipment = Some(*value);
        }
        (None, None) => {}
        _ => anyhow::bail!("incomplete selected equipment baseline"),
    }
    Ok(sources)
}

/// Exact decoded source baselines used to construct applied operating state.
pub(super) struct LoadedSources {
    /// Copied station profile baseline.
    pub profile: Option<clacks_model::profile::SavedLocationProfile>,
    /// Independently copied equipment baseline.
    pub equipment: Option<clacks_model::profile::EquipmentConfiguration>,
}

/// Relational reference to one exact copied source payload.
pub(super) struct StoredSource {
    /// Immutable payload row containing the copied baseline.
    pub payload: i64,
    /// Native source identity qualified by the owning account.
    pub record: String,
}

/// Source references created together for one applied operating context.
pub(super) struct StoredSources {
    /// Exact selected profile reference.
    pub profile: Option<StoredSource>,
    /// Exact selected equipment reference.
    pub equipment: Option<StoredSource>,
}

/// Retain copied source payloads and return their complete relational references.
pub(super) async fn store(
    connection: &mut SqliteConnection,
    account: AccountId,
    state: &super::OperatingState,
) -> anyhow::Result<StoredSources> {
    let profile = if let Some(profile) = &state.selected_profile {
        let key = RecordKey {
            account,
            id: RecordId::Profile(profile.id()),
        };
        Some(StoredSource {
            payload: payload::insert(
                connection,
                key,
                &RecordState::Live(RecordValue::Profile(Box::new(profile.clone()))),
            )
            .await?,
            record: profile.id().to_string(),
        })
    } else {
        None
    };
    let equipment = if let Some(equipment) = &state.selected_equipment {
        let key = RecordKey {
            account,
            id: RecordId::Equipment(equipment.id()),
        };
        Some(StoredSource {
            payload: payload::insert(
                connection,
                key,
                &RecordState::Live(RecordValue::Equipment(Box::new(equipment.clone()))),
            )
            .await?,
            record: equipment.id().to_string(),
        })
    } else {
        None
    };
    Ok(StoredSources { profile, equipment })
}

/// Find actual working tombstones among the source identities used by an Apply.
pub(super) async fn deleted_sources(
    connection: &mut SqliteConnection,
    account: AccountId,
    states: &[&super::OperatingState],
) -> anyhow::Result<std::collections::BTreeSet<RecordId>> {
    let mut ids = std::collections::BTreeSet::new();
    for state in states {
        if let Some(profile) = &state.selected_profile {
            ids.insert(RecordId::Profile(profile.id()));
        }
        if let Some(equipment) = &state.selected_equipment {
            ids.insert(RecordId::Equipment(equipment.id()));
        }
    }
    let mut deleted = std::collections::BTreeSet::new();
    for id in ids {
        if crate::metadata::client::load(connection, RecordKey { account, id })
            .await?
            .as_ref()
            .is_some_and(|record| matches!(record.working_state(), Some(RecordState::Deleted)))
        {
            deleted.insert(id);
        }
    }
    Ok(deleted)
}

/// Remove proven deleted source metadata without touching any copied operating facts.
pub(super) fn forget(
    state: &mut super::OperatingState,
    deleted: &std::collections::BTreeSet<RecordId>,
) {
    if state
        .selected_profile
        .as_ref()
        .is_some_and(|p| deleted.contains(&RecordId::Profile(p.id())))
    {
        state.selected_profile = None;
    }
    if state
        .selected_equipment
        .as_ref()
        .is_some_and(|e| deleted.contains(&RecordId::Equipment(e.id())))
    {
        state.selected_equipment = None;
    }
}

/// Forget a deleted source in the caller's transaction while preserving all copied facts.
pub(crate) async fn forget_source_in(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<()> {
    if !matches!(key.id, RecordId::Profile(_) | RecordId::Equipment(_)) {
        return Ok(());
    }
    let account = key.account.to_string();
    let kind = payload::kind(key.id);
    let id = key.id.to_string();
    sqlx::query_file!("sql/operating/forget_source.sql", account, kind, id)
        .execute(connection)
        .await?;
    Ok(())
}
