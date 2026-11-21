//! Confirmation ownership and staging of the operator's reviewed deletion effects.

use super::*;

/// Explicit replacement choices for the profiles affected by one configuration deletion.
pub(super) struct EquipmentConfirmation {
    /// Configuration and the captured store basis for a persisted deletion.
    target: EquipmentTarget,
    /// Display name of the configuration being removed.
    name: String,
    /// Names and stable identities shown for each referrer.
    profiles: BTreeMap<ProfileId, String>,
    /// Configurations available as replacements in this draft.
    alternatives: BTreeMap<EquipmentConfigurationId, String>,
    /// Each answered profile, distinguishing removal from an unanswered choice.
    choices: BTreeMap<ProfileId, Option<EquipmentConfigurationId>>,
    /// Give Cancel keyboard focus on the first frame.
    focus_cancel: bool,
}

/// A removal of a new draft configuration or a guarded deletion of its persisted record.
enum EquipmentTarget {
    /// Unsaved target referenced by other forms.
    New(EquipmentConfigurationId),
    /// Complete persisted review with projected profiles and replacement bases.
    Saved(Box<EquipmentReview>),
}

impl EquipmentConfirmation {
    /// Show the current reviewed profiles and retain still-available repair choices.
    pub(super) fn new_saved(
        review: EquipmentReview,
        name: String,
        previous: &[ProfileEquipmentReplacement],
    ) -> Self {
        let profiles = review
            .profiles
            .iter()
            .map(|(&id, p)| (id, profile_label(p)))
            .collect();
        let alternatives = review
            .alternatives
            .iter()
            .map(|(&id, e)| (id, equipment_label(e)))
            .collect();
        let choices = previous
            .iter()
            .filter(|item| {
                review.profiles.contains_key(&item.profile)
                    && item
                        .equipment
                        .is_none_or(|id| review.alternatives.contains_key(&id))
            })
            .map(|item| (item.profile, item.equipment))
            .collect();
        Self {
            target: EquipmentTarget::Saved(Box::new(review)),
            name,
            profiles,
            alternatives,
            choices,
            focus_cancel: true,
        }
    }

    /// Review repairs to other raw forms before dropping a newly created configuration.
    pub(super) fn new_local(
        id: EquipmentConfigurationId,
        name: String,
        profiles: BTreeMap<ProfileId, String>,
        alternatives: BTreeMap<EquipmentConfigurationId, String>,
    ) -> Self {
        Self {
            target: EquipmentTarget::New(id),
            name,
            profiles,
            alternatives,
            choices: Default::default(),
            focus_cancel: true,
        }
    }

    /// Present each referring profile's current choice and available replacements.
    fn fields(&mut self, ui: &mut egui::Ui) {
        ui.strong(&self.name);
        ui.label("Choose a replacement or remove the default from each affected profile. Copied operating values are preserved.");
        for (&id, name) in &self.profiles {
            let label = ui.label(name);
            let choice = self.choices.get(&id).copied();
            let caption = match choice {
                None => "Choose treatment".to_owned(),
                Some(None) => "Remove default".to_owned(),
                Some(Some(id)) => self
                    .alternatives
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| format!("Unavailable {id}")),
            };
            egui::ComboBox::from_id_salt(("settings.repair", id))
                .selected_text(caption)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(choice == Some(None), "Remove default")
                        .clicked()
                    {
                        self.choices.insert(id, None);
                    }
                    for (&replacement, name) in &self.alternatives {
                        if ui
                            .selectable_label(choice == Some(Some(replacement)), name)
                            .clicked()
                        {
                            self.choices.insert(id, Some(replacement));
                        }
                    }
                })
                .response
                .labelled_by(label.id);
        }
    }
}

/// A review stays modal until confirmed or explicitly dismissed.
pub(super) enum Review {
    /// Waiting for complete QSO membership from the owning session.
    LoadingTag {
        /// The requested tag identity.
        id: TagId,
        /// Reply belonging only to this review.
        response: oneshot::Receiver<SettingsResponse>,
    },
    /// Consistent tag basis, projected impact counts, and the operator's explicit treatment.
    Tag {
        /// Exact original member and tag records.
        preview: Arc<TagDeletionPreview>,
        /// Counts after prior confirmed deletions in this draft.
        counts: TagDeletionCounts,
        /// Operator's chosen QSO treatment.
        choice: Option<TagDeletionChoice>,
        /// Focus the safe action on entry.
        focus_cancel: bool,
    },
    /// Deleting a reusable station profile preserves copied operating values.
    Profile {
        /// Guard on the profile being removed.
        expected: Box<ClientRecord>,
        /// Name shown to the operator.
        name: String,
        /// Focus the safe action on entry.
        focus_cancel: bool,
    },
    /// Configuration and choices for all projected referrers.
    Equipment(Box<EquipmentConfirmation>),
}

/// Give the safe action the initial focus and preserve the user's subsequent focus.
fn cancel_button(ui: &mut egui::Ui, first_frame: &mut bool) -> bool {
    let response = ui.button("Cancel deletion");
    if std::mem::take(first_frame) {
        response.request_focus();
    }
    response.clicked()
}

impl AccountDraft {
    /// Render one modal and stage its complete review only on explicit confirmation.
    pub(super) fn show_review(&mut self, context: &egui::Context) {
        let Some(mut review) = self.review.take() else {
            return;
        };
        if let Review::LoadingTag { id, response } = &mut review {
            let id = *id;
            match response.try_recv() {
                Ok(response) => {
                    let response = match response {
                        SettingsResponse::RefreshFailed { outcome, .. } => *outcome,
                        response => response,
                    };
                    let loaded = match response {
                        SettingsResponse::Logbooks(LogbookResponse::DeletionPreview(preview))
                            if preview.tag.key().id == RecordId::Tag(id) =>
                        {
                            self.deletions
                                .review_tag(&preview, TagDeletionChoice::LeaveAll)
                                .map(|projection| Review::Tag {
                                    preview,
                                    counts: projection.counts,
                                    choice: self.previous_tag_choices.get(&id).copied(),
                                    focus_cancel: true,
                                })
                        }
                        SettingsResponse::Failed(message) => Err(anyhow::anyhow!(message)),
                        _ => Err(anyhow::anyhow!("The deletion review is unavailable.")),
                    };
                    match loaded {
                        Ok(next) => review = next,
                        Err(error) => {
                            self.review_error = Some((RecordId::Tag(id), error.to_string()));
                            return;
                        }
                    }
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.review_error = Some((
                        RecordId::Tag(id),
                        "The deletion review could not be loaded.".into(),
                    ));
                    return;
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
        }

        let title = match &review {
            Review::Tag { .. } | Review::LoadingTag { .. } => "Delete logbook",
            Review::Equipment(_) => "Delete equipment configuration",
            Review::Profile { .. } => "Delete station profile",
        };
        let mut confirm = false;
        let modal = dialog::show(
            context,
            egui::Id::unique("settings.deletion"),
            title,
            |ui| match &mut review {
                Review::LoadingTag { .. } => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Loading affected contacts…");
                    });
                    ui.button("Cancel deletion").clicked()
                }
                Review::Profile {
                    name, focus_cancel, ..
                } => {
                    ui.strong(name.as_str());
                    ui.label("Applying this deletion removes the reusable profile. Copied station values are preserved.");
                    confirm = ui.button("Stage profile deletion").clicked();
                    cancel_button(ui, focus_cancel)
                }
                Review::Equipment(form) => {
                    form.fields(ui);
                    confirm = ui
                        .add_enabled(
                            form.profiles.keys().all(|id| form.choices.contains_key(id)),
                            egui::Button::new("Confirm deletion"),
                        )
                        .clicked();
                    cancel_button(ui, &mut form.focus_cancel)
                }
                Review::Tag {
                    preview,
                    counts,
                    choice,
                    focus_cancel,
                } => {
                    if let Some(tag) = tag(&preview.tag) {
                        ui.strong(&tag.name);
                    }
                    ui.monospace(preview.tag.key().id.to_string());
                    ui.label(format!(
                        "{} contacts · {} also tagged elsewhere · {} unique to this logbook",
                        counts.affected, counts.also_tagged_elsewhere, counts.unique
                    ));
                    ui.label("Deleting a contact removes it from all logbook views. Preserved contacts lose only this logbook membership.");
                    for (treatment, label, deleted) in [
                        (TagDeletionChoice::LeaveAll, "Leave all contacts", 0),
                        (
                            TagDeletionChoice::DeleteAll,
                            "Delete all contacts",
                            counts.affected,
                        ),
                        (
                            TagDeletionChoice::DeleteUnique,
                            "Delete unique contacts",
                            counts.unique,
                        ),
                    ] {
                        ui.radio_value(
                            choice,
                            Some(treatment),
                            format!(
                                "{label} (delete {deleted}, preserve {})",
                                counts.affected - deleted
                            ),
                        );
                    }
                    confirm = ui
                        .add_enabled(choice.is_some(), egui::Button::new("Confirm deletion"))
                        .clicked();
                    cancel_button(ui, focus_cancel)
                }
            },
        );
        if modal.cancel_requested || modal.inner {
            return;
        }
        if !confirm {
            self.review = Some(review);
            return;
        }

        match review {
            Review::Tag {
                preview,
                choice: Some(choice),
                ..
            } => match self.deletions.review_tag(&preview, choice) {
                Ok(review) => self.deletions.tag_deletions.push(review),
                Err(error) => self.review_error = Some((preview.tag.key().id, error.to_string())),
            },
            Review::Profile { expected, .. } => {
                if let RecordId::Profile(id) = expected.key().id {
                    self.deletions.deleted_profiles.insert(id, *expected);
                }
            }
            Review::Equipment(form) => match form.target {
                EquipmentTarget::New(id) => {
                    for (profile, choice) in form.choices {
                        if let Some(form) = self.profiles.get_mut(&profile) {
                            form.editor.default_equipment = choice;
                        }
                    }
                    self.equipment.remove(&id);
                    self.selected_equipment = None;
                }
                EquipmentTarget::Saved(mut review) => {
                    review.replacements = form
                        .choices
                        .into_iter()
                        .map(|(profile, equipment)| ProfileEquipmentReplacement {
                            profile,
                            equipment,
                        })
                        .collect();
                    self.deletions.equipment_deletions.push(*review);
                }
            },
            _ => {}
        }
    }
}
