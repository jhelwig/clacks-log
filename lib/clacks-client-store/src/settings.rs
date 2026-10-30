//! Account-local reusable settings, consistent source reads and explicit save boundaries.

pub(crate) mod deletion;
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
    /// Workspace-local selected view and current account-owned tag roles.
    pub logbooks: crate::logbooks::LogbookSnapshot,
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
#[tracing::instrument(skip_all, name = "client_store.profile.save", fields(account_id = %store.account, profile_id = %next.id()))]
pub async fn save_profile(
    store: &ClientStore,
    expected: Option<&ClientRecord>,
    next: &SavedLocationProfile,
) -> anyhow::Result<ProfileSaveResult> {
    save::profile(store, expected, next).await
}

/// Save a reusable configuration without loading it or changing any profile association.
#[tracing::instrument(skip_all, name = "client_store.equipment.save", fields(account_id = %store.account, equipment_id = %next.id()))]
pub async fn save_equipment(
    store: &ClientStore,
    expected: Option<&ClientRecord>,
    next: &EquipmentConfiguration,
) -> anyhow::Result<SaveOutcome> {
    save::equipment(store, expected, next).await
}

/// Exact equipment, referring profiles and available replacements reviewed for deletion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EquipmentDeletionPreview {
    /// Live configuration being considered for deletion.
    pub equipment: ClientRecord,
    /// Complete locally visible working set referring to this configuration.
    pub referring_profiles: Vec<ClientRecord>,
    /// Live alternative configurations captured in the same snapshot.
    pub replacement_equipment: Vec<ClientRecord>,
}

/// Explicit treatment of one profile's equipment default during deletion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileEquipmentReplacement {
    /// Profile whose default is being repaired.
    pub profile: clacks_model::ids::ProfileId,
    /// Reviewed replacement, or explicit removal of the default.
    pub equipment: Option<EquipmentConfigurationId>,
}

/// Result of rechecking a complete equipment-deletion review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EquipmentDeleteOutcome {
    /// All profile repairs and the deletion committed together.
    Applied,
    /// Reviewed state changed; no deletion or repair was committed.
    Stale(EquipmentDeletionPreview),
    /// Target no longer has a usable live working configuration.
    Unavailable {
        /// Actual local record, when known; absence is not proof of deletion.
        current: Option<ClientRecord>,
    },
}

/// Delete a station profile without changing copied station facts or selected equipment.
#[tracing::instrument(skip_all, name = "client_store.profile.delete", fields(account_id = %store.account, profile_id = %expected.key.id))]
pub async fn delete_profile(
    store: &ClientStore,
    expected: &ClientRecord,
) -> anyhow::Result<SaveOutcome> {
    anyhow::ensure!(
        matches!(expected.key.id, clacks_sync::record::RecordId::Profile(_)),
        "profile deletion requires a profile record"
    );
    crate::save::save(
        store,
        expected.key,
        Some(expected),
        clacks_sync::record::RecordState::Deleted,
    )
    .await
}

/// Review all current referring profiles and available replacement configurations consistently.
pub async fn preview_equipment_deletion(
    store: &ClientStore,
    id: EquipmentConfigurationId,
) -> anyhow::Result<EquipmentDeletionPreview> {
    deletion::preview(store, id).await
}

/// Commit reviewed profile-reference repairs and equipment deletion atomically, or report stale state.
#[tracing::instrument(skip_all, name = "client_store.equipment.delete", fields(account_id = %store.account, equipment_id = %expected.equipment.key.id))]
pub async fn delete_equipment(
    store: &ClientStore,
    expected: &EquipmentDeletionPreview,
    replacements: &[ProfileEquipmentReplacement],
) -> anyhow::Result<EquipmentDeleteOutcome> {
    deletion::apply(store, expected, replacements).await
}
