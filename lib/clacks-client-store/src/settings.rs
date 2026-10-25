//! Account-local reusable settings, consistent source reads and explicit save boundaries.

pub(crate) mod read;
mod save;

use crate::{operating::OperatingState, save::SaveOutcome, store::ClientStore};
use clacks_model::{
    ids::EquipmentConfigurationId,
    profile::{EquipmentConfiguration, SavedLocationProfile},
};
use clacks_sync::client::ClientRecord;

/// One consistent local view of applied settings and reusable working records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsSnapshot {
    /// Applied facts and their independent optional source selections.
    pub operating: OperatingState,
    /// Complete profile roles, including retained working deletions.
    pub profiles: Vec<ClientRecord>,
    /// Complete equipment roles, including retained working deletions.
    pub equipment: Vec<ClientRecord>,
    /// Account-owned reference definitions and their working state.
    pub groups: Vec<ClientRecord>,
}

/// A guarded profile save plus a reference that needs explicit local repair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileSaveResult {
    /// Saved, unchanged or stale shared-record outcome.
    pub outcome: SaveOutcome,
    /// Equipment not currently available as a live local configuration.
    pub missing_equipment: Option<EquipmentConfigurationId>,
}

/// Read applied settings and account-shared sources in one bounded database snapshot.
pub async fn snapshot(store: &ClientStore) -> anyhow::Result<SettingsSnapshot> {
    let mut read = crate::read::snapshot(store).await?;
    let value = read.settings().await?;
    read.close().await?;
    Ok(value)
}

/// Save a profile without applying its values, preserving missing references as pending work.
#[tracing::instrument(skip_all, name = "client_store.profile.save", fields(account_id = %store.account, profile_id = %next.id))]
pub async fn save_profile(
    store: &ClientStore,
    expected: Option<&ClientRecord>,
    next: &SavedLocationProfile,
) -> anyhow::Result<ProfileSaveResult> {
    save::profile(store, expected, next).await
}

/// Save a reusable configuration without loading it or changing any profile association.
#[tracing::instrument(skip_all, name = "client_store.equipment.save", fields(account_id = %store.account, equipment_id = %next.id))]
pub async fn save_equipment(
    store: &ClientStore,
    expected: Option<&ClientRecord>,
    next: &EquipmentConfiguration,
) -> anyhow::Result<SaveOutcome> {
    save::equipment(store, expected, next).await
}
