//! Unapplied account record forms, guarded deletion reviews, and record-local feedback.

mod logbooks;
mod reviews;
mod stations;
#[cfg(test)]
mod tests;

use crate::{
    geography::GeographyStatus,
    logbooks::TagEditor,
    operating::{
        FieldError,
        editor::{EquipmentEditor, ProfileEditor},
        equipment::EquipmentForm,
        station::StationForm,
    },
    session::{
        SessionCommand,
        settings::{
            SettingsAction, SettingsRequest, SettingsResponse,
            apply::{AccountApplyResult, AccountChanges, AccountOutcome, Edit, EquipmentReview},
            logbooks::{LogbookAction, LogbookResponse},
        },
    },
    ui::{
        dialog, equipment, feedback,
        geography::GeographyView,
        logbooks::{editor, labels, tag},
        operating::{configuration, equipment_label, profile, profile_label},
        profiles,
    },
};
use clacks_client_store::{
    logbooks::deletion::{TagDeletionCounts, TagDeletionPreview},
    settings::{ProfileEquipmentReplacement, SettingsSnapshot},
};
use clacks_model::{
    group::DefinitionSet,
    ids::{EquipmentConfigurationId, ProfileId, TagId},
    tag::{LogbookTag, TagDeletionChoice},
};
use clacks_sync::{client::ClientRecord, record::RecordId};
use eframe::egui;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{mpsc, oneshot};

/// A named-logbook form and the complete local record it was edited against.
struct TagForm {
    /// Guard for updates, absent for creation.
    expected: Option<ClientRecord>,
    /// Raw form fields, including incomplete names.
    editor: TagEditor,
}

/// Station form and geographic assistance for only this profile.
struct StationFormDraft {
    /// Editable fields and original record basis.
    editor: ProfileEditor,
    /// Lookup ownership follows the station position being edited.
    geography: GeographyView,
}

/// The record's last unsuccessful Apply, with the attempted identity for failed creations.
struct Problem {
    /// Display label retained even when the target has no current record.
    name: String,
    /// Validation or store feedback.
    message: String,
}

/// Live validation state for one visible form.
#[derive(Default)]
struct LiveValidation {
    /// The next time raw edits should be checked.
    state: crate::ui::validation::ValidationState,
    /// Last settled field errors.
    errors: Vec<FieldError>,
}

impl LiveValidation {
    /// Advance validation using the current frame's input clock.
    fn advance(&mut self, ui: &egui::Ui, check: impl FnOnce() -> Vec<FieldError>) {
        let now = ui.input(|input| input.time);
        if self.state.due(now) {
            self.errors = check();
            self.state.validated();
        }
        self.state.request_repaint(ui.ctx(), now);
    }

    /// Schedule validation after the user finishes the current edit.
    fn edited(&mut self, ui: &egui::Ui) {
        let now = ui.input(|input| input.time);
        self.state.edited(now);
        self.state.request_repaint(ui.ctx(), now);
    }
}

/// Per-record raw edits and confirmations, retained when changing Settings sections.
#[derive(Default)]
pub(super) struct AccountDraft {
    /// Changed or new logbooks.
    tags: BTreeMap<TagId, TagForm>,
    /// Changed or new station profiles.
    profiles: BTreeMap<ProfileId, StationFormDraft>,
    /// Changed or new equipment configurations.
    equipment: BTreeMap<EquipmentConfigurationId, EquipmentEditor>,
    /// Confirmed deletions, in review order, including profile reference repairs.
    deletions: AccountChanges,
    /// Which logbook form is visible.
    selected_tag: Option<TagId>,
    /// Station profile selected in its section.
    selected_profile: Option<ProfileId>,
    /// Equipment configuration selected in its section.
    selected_equipment: Option<EquipmentConfigurationId>,
    /// A modal confirmation or pending read of its impact.
    review: Option<reviews::Review>,
    /// Field feedback and debounce deadlines follow the edited record.
    validation: BTreeMap<RecordId, LiveValidation>,
    /// Confirmed effects of the last logbook deletions, retained after the deleted record disappears.
    deleted_logbooks: Vec<String>,
    /// Explanations attached to records after Apply.
    problems: BTreeMap<RecordId, Problem>,
    /// Names and IDs that were submitted.
    submitted: BTreeMap<RecordId, String>,
    /// Previous treatment retained when a stale tag deletion is reviewed again.
    previous_tag_choices: BTreeMap<TagId, TagDeletionChoice>,
    /// Still-applicable repairs can be prefilled for a new review.
    previous_repairs: BTreeMap<EquipmentConfigurationId, Vec<ProfileEquipmentReplacement>>,
    /// Transient review-loading failure, presented next to the target.
    review_error: Option<(RecordId, String)>,
}

impl AccountDraft {
    /// Open the profile identified by a workbench repair action.
    pub(super) fn open_profile(&mut self, id: ProfileId) {
        self.selected_profile = Some(id);
    }

    /// Whether any account change or incomplete new record is staged.
    pub(super) fn dirty(&self) -> bool {
        !self.tags.is_empty()
            || !self.equipment.is_empty()
            || !self.profiles.is_empty()
            || !self.deletions.tag_deletions.is_empty()
            || !self.deletions.equipment_deletions.is_empty()
            || !self.deletions.deleted_profiles.is_empty()
    }

    /// Whether Escape belongs to an open review before the surrounding Settings screen.
    pub(super) fn reviewing(&self) -> bool {
        self.review.is_some()
    }

    /// Whether any setting in the requested record family has a failed Apply result.
    pub(super) fn section_failed(&self, contains: impl Fn(RecordId) -> bool) -> bool {
        self.problems.keys().copied().any(contains)
    }

    /// Build valid upserts against the original records independently of other forms' errors.
    fn changes(
        &self,
        definitions: &DefinitionSet,
    ) -> (AccountChanges, BTreeMap<RecordId, Vec<FieldError>>) {
        let mut changes = self.deletions.clone();
        let mut invalid = BTreeMap::new();
        for (&id, form) in &self.tags {
            if changes.tag_deletions.iter().any(|review| review.id == id) {
                continue;
            }
            match form.editor.build() {
                Ok(next) => {
                    changes.tags.insert(
                        id,
                        Edit {
                            expected: form.expected.clone(),
                            next,
                        },
                    );
                }
                Err(errors) => {
                    invalid.insert(RecordId::Tag(id), errors);
                }
            }
        }
        for (&id, form) in &self.equipment {
            if changes
                .equipment_deletions
                .iter()
                .any(|review| review.id == id)
            {
                continue;
            }
            match form.build() {
                Ok(next) => {
                    changes.equipment.insert(
                        id,
                        Edit {
                            expected: form.expected.clone(),
                            next,
                        },
                    );
                }
                Err(errors) => {
                    invalid.insert(RecordId::Equipment(id), errors);
                }
            }
        }
        for (&id, form) in &self.profiles {
            if changes.deleted_profiles.contains_key(&id) {
                continue;
            }
            match form.editor.build(definitions) {
                Ok(next) => {
                    changes.profiles.insert(
                        id,
                        Edit {
                            expected: form.editor.expected.clone(),
                            next,
                        },
                    );
                }
                Err(errors) => {
                    invalid.insert(RecordId::Profile(id), errors);
                }
            }
        }
        (changes, invalid)
    }

    /// Submit each valid record independently and capture diagnostics for incomplete input.
    pub(super) fn submit(
        &mut self,
        snapshot: &SettingsSnapshot,
        definitions: &DefinitionSet,
    ) -> AccountChanges {
        let (changes, invalid) = self.changes(definitions);
        self.problems.clear();
        self.deleted_logbooks.clear();
        self.previous_tag_choices.clear();
        self.previous_repairs.clear();
        self.submitted = self
            .tags
            .iter()
            .map(|(id, form)| (RecordId::Tag(*id), form.editor.name.clone()))
            .chain(
                self.equipment
                    .iter()
                    .map(|(id, form)| (RecordId::Equipment(*id), form.name.clone())),
            )
            .chain(
                self.profiles
                    .iter()
                    .map(|(id, form)| (RecordId::Profile(*id), form.editor.name.clone())),
            )
            .collect();

        let deletions = changes
            .tag_deletions
            .iter()
            .map(|review| RecordId::Tag(review.id))
            .chain(
                changes
                    .equipment_deletions
                    .iter()
                    .map(|review| RecordId::Equipment(review.id)),
            )
            .chain(
                changes
                    .deleted_profiles
                    .keys()
                    .copied()
                    .map(RecordId::Profile),
            );
        for id in deletions {
            self.submitted.entry(id).or_insert_with(|| {
                let name = match id {
                    RecordId::Tag(id) => snapshot
                        .logbooks
                        .tags
                        .iter()
                        .filter_map(tag)
                        .find(|tag| tag.id == id)
                        .map(|tag| tag.name.as_str()),
                    RecordId::Equipment(id) => snapshot
                        .equipment
                        .iter()
                        .filter_map(configuration)
                        .find(|value| value.id() == id)
                        .map(|value| value.name()),
                    RecordId::Profile(id) => changes
                        .deleted_profiles
                        .get(&id)
                        .and_then(profile)
                        .map(|value| value.name()),
                    _ => None,
                };
                name.map(str::to_owned).unwrap_or_else(|| id.to_string())
            });
        }
        for (id, errors) in invalid {
            self.problem(
                id,
                errors
                    .iter()
                    .map(|e| format!("{}: {}", e.field, e.message))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        changes
    }

    /// Attach an unsuccessful operation to the record that was attempted.
    fn problem(&mut self, id: RecordId, message: String) {
        self.problems.insert(
            id,
            Problem {
                name: self.submitted.get(&id).cloned().unwrap_or_default(),
                message,
            },
        );
    }

    /// Attach a lost response or failed admission to each valid record in the submitted request.
    pub(super) fn fail_submitted(&mut self, message: &str) {
        for (&id, name) in &self.submitted {
            self.problems.entry(id).or_insert_with(|| Problem {
                name: name.clone(),
                message: message.to_owned(),
            });
        }
    }

    /// Consume the account worker's per-operation results and discard the submitted forms.
    pub(super) fn settled(&mut self, result: AccountApplyResult) {
        for (id, outcome) in result.outcomes {
            if let AccountOutcome::DeletedLogbook {
                plan,
                fallback_from,
            } = &outcome
            {
                let name = self
                    .submitted
                    .get(&id)
                    .filter(|name| !name.is_empty())
                    .cloned()
                    .unwrap_or_else(|| id.to_string());
                let mut message = format!(
                    "Deleted {name}. Contacts deleted: {}. Contacts kept with this logbook removed: {}.",
                    plan.mark_deleted.len(),
                    plan.remove_membership.len()
                );
                if fallback_from.is_some() {
                    message.push_str(" The current view is now All contacts.");
                }
                self.deleted_logbooks.push(message);
            }
            if let Some(message) = outcome.diagnostic() {
                self.problem(id, message);
                match id {
                    RecordId::Tag(id) => {
                        if let Some(review) = self
                            .deletions
                            .tag_deletions
                            .iter()
                            .find(|review| review.id == id)
                        {
                            self.previous_tag_choices.insert(id, review.choice);
                        }
                    }
                    RecordId::Equipment(id) => {
                        if let Some(review) = self
                            .deletions
                            .equipment_deletions
                            .iter()
                            .find(|review| review.id == id)
                        {
                            self.previous_repairs
                                .insert(id, review.replacements.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        self.clear_forms();
    }

    /// Reset only unapplied account inputs while keeping the current destination and results.
    pub(super) fn clear_forms(&mut self) {
        self.tags.clear();
        self.profiles.clear();
        self.equipment.clear();
        self.deletions = AccountChanges::default();
        self.review = None;
        self.review_error = None;
        self.submitted.clear();
        self.validation.clear();
    }

    /// Show the result of a failed attempt beside its affected record.
    fn diagnostics(&self, ui: &mut egui::Ui, id: RecordId) {
        if let Some(problem) = self.problems.get(&id) {
            let name = if problem.name.trim().is_empty() {
                match id {
                    RecordId::Tag(_) => "New logbook",
                    RecordId::Profile(_) => "New profile",
                    RecordId::Equipment(_) => "New configuration",
                    _ => "Setting",
                }
            } else {
                &problem.name
            };
            feedback::problem(
                ui,
                &format!("⚠ Could not apply {name}.\n{}", problem.message),
            );
        }
        if let Some((target, message)) = &self.review_error
            && *target == id
        {
            feedback::problem(ui, message);
        }
    }
}
