//! Guarded account changes submitted together from the Settings editor.

mod review;
#[cfg(test)]
mod tests;

use clacks_client_store::{
    logbooks::{
        deletion::{self, TagDeleteOutcome, TagDeletionCounts, TagDeletionPreview},
        selection::SelectedView,
        tags,
    },
    save::SaveOutcome,
    settings::{
        self, EquipmentDeleteOutcome, EquipmentDeletionPreview, ProfileEquipmentReplacement,
        SettingsSnapshot,
    },
    store::ClientStore,
};
use clacks_model::{
    contact::Qso,
    ids::{EquipmentConfigurationId, ProfileId, QsoId, TagId},
    profile::{EquipmentConfiguration, SavedLocationProfile},
    tag::{LogbookTag, TagDeletionChoice, TagDeletionPlan},
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordState, RecordValue},
};
use std::{collections::BTreeMap, sync::Arc};

/// A validated desired value and the exact record from which editing began.
#[derive(Clone, Debug)]
pub(crate) struct Edit<T> {
    /// Original record, absent for a newly allocated identity.
    pub expected: Option<ClientRecord>,
    /// Desired native value.
    pub next: T,
}

/// Final typed operations for one Apply, with destructive reviews in confirmation order.
#[derive(Clone, Debug, Default)]
pub(crate) struct AccountChanges {
    /// Logbook names and automatic-upload policies.
    pub tags: BTreeMap<TagId, Edit<LogbookTag>>,
    /// Reusable equipment configurations.
    pub equipment: BTreeMap<EquipmentConfigurationId, Edit<EquipmentConfiguration>>,
    /// Reusable station profiles and their optional equipment defaults.
    pub profiles: BTreeMap<ProfileId, Edit<SavedLocationProfile>>,
    /// Confirmed deletions of persisted station profiles.
    pub deleted_profiles: BTreeMap<ProfileId, ClientRecord>,
    /// Approved logbook deletions, including their projected QSO effects.
    pub tag_deletions: Vec<TagReview>,
    /// Approved equipment deletions and profile treatment choices.
    pub equipment_deletions: Vec<EquipmentReview>,
}

impl AccountChanges {
    /// Whether this submission has any shared-record work.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
            && self.equipment.is_empty()
            && self.profiles.is_empty()
            && self.deleted_profiles.is_empty()
            && self.tag_deletions.is_empty()
            && self.equipment_deletions.is_empty()
    }
}

/// The exact projected memberships shown for one logbook deletion choice.
#[derive(Clone, Debug)]
pub(crate) struct TagReview {
    /// Logbook whose deletion was reviewed.
    pub id: TagId,
    /// Projected counts shown before confirmation.
    pub counts: TagDeletionCounts,
    /// Confirmed contact treatment.
    pub choice: TagDeletionChoice,
    /// Original tag and member records as read from the workspace.
    basis: Arc<TagDeletionPreview>,
    /// Changed QSO values expected from earlier membership removals.
    projected: BTreeMap<QsoId, RecordValue>,
    /// Identity-specific effects after previous reviewed operations.
    plan: TagDeletionPlan,
}

/// The projected profile references and available equipment shown for one deletion.
#[derive(Clone, Debug)]
pub(crate) struct EquipmentReview {
    /// Equipment whose deletion was reviewed.
    pub id: EquipmentConfigurationId,
    /// Profiles expected to refer to the target when this operation runs.
    pub profiles: BTreeMap<ProfileId, SavedLocationProfile>,
    /// Live replacement choices, including staged configurations.
    pub alternatives: BTreeMap<EquipmentConfigurationId, EquipmentConfiguration>,
    /// Confirmed choice for each referring profile.
    pub replacements: Vec<ProfileEquipmentReplacement>,
    /// Exact target record presented for deletion.
    target: ClientRecord,
    /// Persisted bases for projected profiles and replacement equipment.
    bases: BTreeMap<RecordId, ClientRecord>,
}

/// Authoritative disposition of one staged setting.
#[derive(Debug)]
pub(crate) enum AccountOutcome {
    /// A changed value or deletion committed.
    Saved {
        /// A saved profile still references unavailable equipment.
        missing_equipment: Option<EquipmentConfigurationId>,
    },
    /// A logbook and the confirmed contacts or memberships were removed.
    DeletedLogbook {
        /// Actual contact effects committed by the guarded deletion.
        plan: TagDeletionPlan,
        /// Deleted selection whose committed fallback is All contacts.
        fallback_from: Option<TagId>,
    },
    /// The guarded record already held this value.
    Unchanged {
        /// A saved profile still references unavailable equipment.
        missing_equipment: Option<EquipmentConfigurationId>,
    },
    /// The target or confirmed destructive impact changed before execution.
    Rejected(String),
    /// An operation ended without an authoritative acknowledgment.
    Unverified(String),
}

impl AccountOutcome {
    /// Return a local diagnostic for a failed operation or pending profile repair.
    pub fn diagnostic(&self) -> Option<String> {
        match self {
            Self::Saved { missing_equipment } | Self::Unchanged { missing_equipment } => {
                missing_equipment
                    .map(|id| format!("Profile saved. Its default equipment {id} is unavailable."))
            }
            Self::DeletedLogbook { .. } => None,
            Self::Rejected(message) => Some(message.clone()),
            Self::Unverified(message) => Some(format!("Could not verify the change. {message}")),
        }
    }
}

/// Exact record transition acknowledged by one or more operations in this Apply.
#[derive(Debug)]
struct Transition {
    /// Record before the first successful operation, absent for a new identity.
    original: Option<ClientRecord>,
    /// Last authoritative record returned by the owning transaction.
    current: ClientRecord,
}

/// Independent results and authoritative changes available before any follow-up read.
#[derive(Default, Debug)]
pub(crate) struct AccountApplyResult {
    /// Outcome keyed by the edited or deleted setting's stable identity.
    pub outcomes: BTreeMap<RecordId, AccountOutcome>,
    /// Records acknowledged across the submission in their final committed state.
    records: BTreeMap<RecordId, Transition>,
    /// Current settings returned when a guarded write found a different basis.
    observed: BTreeMap<RecordId, Option<ClientRecord>>,
    /// Last selected view returned by a successful logbook deletion.
    selected_view: Option<SelectedView>,
}

impl AccountApplyResult {
    /// Apply confirmed changes to the prior view while a fresh workspace read is pending.
    pub fn update_snapshot(&self, snapshot: &mut SettingsSnapshot) {
        if let Some(view) = self.selected_view {
            snapshot.logbooks.selected_view = view;
        }
        let acknowledged = self
            .records
            .iter()
            .map(|(id, change)| (id, Some(&change.current)));
        let observed = self.observed.iter().map(|(id, row)| (id, row.as_ref()));
        for (id, current) in acknowledged.chain(observed) {
            let catalog = match id {
                RecordId::Tag(_) => &mut snapshot.logbooks.tags,
                RecordId::Profile(_) => &mut snapshot.profiles,
                RecordId::Equipment(_) => &mut snapshot.equipment,
                _ => continue,
            };
            let Some(current) = current else {
                catalog.retain(|row| row.key().id != *id);
                continue;
            };
            if matches!(current.working_state(), Some(RecordState::Deleted)) {
                snapshot.operating.forget_source(*id);
            }
            if let Some(prior) = catalog.iter_mut().find(|row| row.key() == current.key()) {
                prior.clone_from(current);
            } else {
                catalog.push(current.clone());
            }
        }
    }

    /// Preserve the original basis when several guarded operations affect the same record.
    fn acknowledge(&mut self, original: Option<&ClientRecord>, current: ClientRecord) {
        self.observed.remove(&current.key().id);
        match self.records.entry(current.key().id) {
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                entry.get_mut().current = current
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Transition {
                    original: original.cloned(),
                    current,
                });
            }
        }
    }

    /// Resolve projected values only through their original basis or a confirmed transition from it.
    fn reviewed(
        &self,
        id: RecordId,
        basis: Option<&ClientRecord>,
        value: &RecordValue,
    ) -> Option<ClientRecord> {
        let row = match self.records.get(&id) {
            Some(transition) if transition.original.as_ref() == basis => &transition.current,
            Some(_) => return None,
            None => basis?,
        };
        matches!(row.working_state(), Some(RecordState::Live(current)) if current == value)
            .then(|| row.clone())
    }

    /// Record a single guarded save and any separately reported equipment-reference issue.
    fn save(
        &mut self,
        id: RecordId,
        basis: Option<&ClientRecord>,
        result: anyhow::Result<SaveOutcome>,
        missing_equipment: Option<EquipmentConfigurationId>,
    ) {
        let outcome = match result {
            Ok(SaveOutcome::Saved(row)) => {
                self.acknowledge(basis, row);
                AccountOutcome::Saved { missing_equipment }
            }
            Ok(SaveOutcome::Unchanged(row)) => {
                self.acknowledge(basis, row);
                AccountOutcome::Unchanged { missing_equipment }
            }
            Ok(SaveOutcome::Stale { current }) => {
                self.observed.insert(id, current);
                AccountOutcome::Rejected(
                    "This record changed. Review its saved values and try again.".into(),
                )
            }
            Err(error) => AccountOutcome::Unverified(format!("{error:#}")),
        };
        self.outcomes.insert(id, outcome);
    }
}

/// Save independent changes in dependency order, rechecking each reviewed deletion atomically.
pub(super) async fn execute(store: &ClientStore, changes: AccountChanges) -> AccountApplyResult {
    let mut result = AccountApplyResult::default();
    for (id, edit) in changes.tags {
        let saved = tags::save(store, edit.expected.as_ref(), &edit.next).await;
        result.save(RecordId::Tag(id), edit.expected.as_ref(), saved, None);
    }
    for (id, edit) in changes.equipment {
        let saved = settings::save_equipment(store, edit.expected.as_ref(), &edit.next).await;
        result.save(RecordId::Equipment(id), edit.expected.as_ref(), saved, None);
    }
    for (id, edit) in changes.profiles {
        let saved = settings::save_profile(store, edit.expected.as_ref(), &edit.next).await;
        let missing = saved
            .as_ref()
            .ok()
            .and_then(|report| report.missing_equipment);
        result.save(
            RecordId::Profile(id),
            edit.expected.as_ref(),
            saved.map(|report| report.outcome),
            missing,
        );
    }
    for (id, basis) in changes.deleted_profiles {
        let saved = settings::delete_profile(store, &basis).await;
        result.save(RecordId::Profile(id), Some(&basis), saved, None);
    }
    for review in changes.tag_deletions {
        let outcome = review.apply(store, &mut result).await;
        result.outcomes.insert(RecordId::Tag(review.id), outcome);
    }
    for review in changes.equipment_deletions {
        let outcome = review.apply(store, &mut result).await;
        result
            .outcomes
            .insert(RecordId::Equipment(review.id), outcome);
    }
    result
        .records
        .retain(|id, _| !matches!(id, RecordId::Qso(_)));
    result
}
