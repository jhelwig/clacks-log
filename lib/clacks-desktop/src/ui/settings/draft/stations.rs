//! Reusable records, including references to equipment created in the same draft.

use super::*;

impl AccountDraft {
    /// Present the station profile catalog and selected profile's fields.
    pub(in crate::ui::settings) fn station_profiles(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &SettingsSnapshot,
        definitions: &DefinitionSet,
        commands: &mpsc::Sender<SessionCommand>,
        geography: &GeographyStatus,
    ) {
        for (id, form) in &mut self.profiles {
            if form.geography.advance(
                &mut form.editor.station,
                commands,
                geography,
                !ui.is_enabled(),
                "station_profile",
            ) {
                self.validation
                    .entry(RecordId::Profile(*id))
                    .or_default()
                    .edited(ui);
            }
        }
        let mut profiles: BTreeMap<_, _> = snapshot
            .profiles
            .iter()
            .filter_map(profile)
            .map(|value| (value.id(), profile_label(value)))
            .collect();
        profiles.extend(self.profiles.iter().map(|(&id, form)| {
            (
                id,
                format!(
                    "{} · {} · {id}",
                    form.editor.name, form.editor.station.callsign
                ),
            )
        }));
        if ui.button("New profile").clicked() {
            let editor = ProfileEditor::new(None, StationForm::empty());
            let id = editor.id;
            self.profiles.insert(
                id,
                StationFormDraft {
                    editor,
                    geography: Default::default(),
                },
            );
            self.open_profile(id);
        }
        for (id, label) in &profiles {
            let key = RecordId::Profile(*id);
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.selected_profile, Some(*id), label);
                if self.deletions.deleted_profiles.contains_key(id) {
                    ui.small("Deletion staged");
                }
            });
            self.diagnostics(ui, key);
        }
        for id in self.problems.keys() {
            if let RecordId::Profile(profile) = id
                && !profiles.contains_key(profile)
            {
                self.diagnostics(ui, *id);
            }
        }
        if let Some(id) = self.selected_profile {
            ui.separator();
            if self.deletions.deleted_profiles.contains_key(&id) {
                if ui.button("Undo staged deletion").clicked() {
                    self.deletions.deleted_profiles.remove(&id);
                }
            } else {
                let mut configurations = self.equipment_catalog(snapshot);
                for review in &self.deletions.equipment_deletions {
                    configurations.remove(&review.id);
                }
                self.profile_form(ui, snapshot, id, &configurations, definitions);
            }
        }
        self.show_review(ui.ctx());
    }

    /// Present reusable equipment and the selected configuration's fields.
    pub(in crate::ui::settings) fn equipment_configurations(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &SettingsSnapshot,
        definitions: &DefinitionSet,
    ) {
        if ui.button("New configuration").clicked() {
            let editor = EquipmentEditor::new(None, EquipmentForm::default());
            let id = editor.id;
            self.equipment.insert(id, editor);
            self.selected_equipment = Some(id);
        }
        let configurations = self.equipment_catalog(snapshot);
        for (id, label) in &configurations {
            let key = RecordId::Equipment(*id);
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.selected_equipment, Some(*id), label);
                if self
                    .deletions
                    .equipment_deletions
                    .iter()
                    .any(|review| review.id == *id)
                {
                    ui.small("Deletion staged");
                }
            });
            self.diagnostics(ui, key);
        }
        for id in self.problems.keys() {
            if let RecordId::Equipment(equipment) = id
                && !configurations.contains_key(equipment)
            {
                self.diagnostics(ui, *id);
            }
        }
        if let Some(id) = self.selected_equipment {
            ui.separator();
            if self
                .deletions
                .equipment_deletions
                .iter()
                .any(|review| review.id == id)
            {
                if ui.button("Undo staged deletion").clicked() {
                    self.deletions
                        .equipment_deletions
                        .retain(|review| review.id != id);
                }
            } else {
                self.equipment_form(ui, snapshot, id, definitions, &configurations);
            }
        }
        self.show_review(ui.ctx());
    }

    /// Equipment choices with staged names and records included.
    fn equipment_catalog(
        &self,
        snapshot: &SettingsSnapshot,
    ) -> BTreeMap<EquipmentConfigurationId, String> {
        let mut configurations: BTreeMap<_, _> = snapshot
            .equipment
            .iter()
            .filter_map(configuration)
            .map(|value| (value.id(), equipment_label(value)))
            .collect();
        configurations.extend(
            self.equipment
                .iter()
                .map(|(&id, form)| (id, format!("{} · {id}", form.name))),
        );
        configurations
    }

    /// Edit the selected profile against its original basis, including projected equipment choices.
    fn profile_form(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &SettingsSnapshot,
        id: ProfileId,
        options: &BTreeMap<EquipmentConfigurationId, String>,
        definitions: &DefinitionSet,
    ) {
        let key = RecordId::Profile(id);
        let form = self.profiles.remove(&id);
        let was_dirty = form.is_some();
        let form = form.or_else(|| {
            let expected = snapshot.profiles.iter().find(|row| row.key().id == key)?;
            let station = StationForm::from_snapshot(profile(expected)?.station());
            Some(StationFormDraft {
                editor: ProfileEditor::new(Some(expected.clone()), station),
                geography: Default::default(),
            })
        });
        let Some(mut form) = form else {
            ui.label(format!("Station profile {id} is unavailable."));
            return;
        };
        let validation = self.validation.entry(key).or_default();
        let editor = &mut form.editor;
        validation.advance(ui, || editor.build(definitions).err().unwrap_or_default());

        let mut changed = profiles::name_field(ui, editor, &validation.errors);
        let mut projected = editor.default_equipment;
        for review in &self.deletions.equipment_deletions {
            if projected == Some(review.id)
                && let Some(repair) = review
                    .replacements
                    .iter()
                    .find(|repair| repair.profile == id)
            {
                projected = repair.equipment;
            }
        }
        let before = projected;
        equipment::association(
            ui,
            "profile.default",
            &mut projected,
            options.iter().map(|(id, label)| (*id, label.clone())),
        );
        if projected != before {
            editor.default_equipment = projected;
            changed = true;
        }
        changed |= profiles::station_fields(ui, editor, definitions, &validation.errors);
        changed |= form.geography.show(ui, &mut editor.station);
        if changed {
            validation.edited(ui);
        }

        let new = editor.expected.is_none();
        if ui
            .button(if new {
                "Discard new profile"
            } else {
                "Delete profile"
            })
            .clicked()
        {
            if let Some(expected) = editor.expected.clone() {
                self.review = Some(reviews::Review::Profile {
                    expected: Box::new(expected),
                    name: editor.name.clone(),
                    focus_cancel: true,
                });
            } else {
                self.selected_profile = None;
            }
            if new {
                return;
            }
        }
        if (was_dirty || changed)
            && (new
                || editor.build(definitions).ok().as_ref()
                    != editor.expected.as_ref().and_then(profile))
        {
            self.profiles.insert(id, form);
        }
    }

    /// Edit one configuration and review profile repairs before staging a removal.
    fn equipment_form(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &SettingsSnapshot,
        id: EquipmentConfigurationId,
        definitions: &DefinitionSet,
        options: &BTreeMap<EquipmentConfigurationId, String>,
    ) {
        let key = RecordId::Equipment(id);
        let form = self.equipment.remove(&id);
        let was_dirty = form.is_some();
        let form = form.or_else(|| {
            let expected = snapshot.equipment.iter().find(|row| row.key().id == key)?;
            let values = EquipmentForm::from_values(Some(configuration(expected)?.values()));
            Some(EquipmentEditor::new(Some(expected.clone()), values))
        });
        let Some(mut form) = form else {
            ui.label(format!("Equipment configuration {id} is unavailable."));
            return;
        };
        let validation = self.validation.entry(key).or_default();
        validation.advance(ui, || form.build().err().unwrap_or_default());
        let changed = equipment::fields(ui, &mut form, &validation.errors);
        if changed {
            validation.edited(ui);
        }
        let new = form.expected.is_none();
        let name = form.name.clone();
        if (was_dirty || changed)
            && (new || form.build().ok().as_ref() != form.expected.as_ref().and_then(configuration))
        {
            self.equipment.insert(id, form);
        }
        let button = if new {
            "Discard new configuration"
        } else if self.previous_repairs.contains_key(&id) {
            "Review updated deletion"
        } else {
            "Review deletion"
        };
        if ui.button(button).clicked() {
            self.review_error = None;
            if new {
                let profiles: BTreeMap<_, _> = self
                    .profiles
                    .iter()
                    .filter(|(profile, form)| {
                        form.editor.default_equipment == Some(id)
                            && !self.deletions.deleted_profiles.contains_key(profile)
                    })
                    .map(|(&id, form)| (id, form.editor.name.clone()))
                    .collect();
                if profiles.is_empty() {
                    self.equipment.remove(&id);
                    self.selected_equipment = None;
                } else {
                    let alternatives = options
                        .iter()
                        .filter(|(other, _)| {
                            **other != id
                                && !self
                                    .deletions
                                    .equipment_deletions
                                    .iter()
                                    .any(|review| review.id == **other)
                        })
                        .map(|(id, label)| (*id, label.clone()))
                        .collect();
                    self.review = Some(reviews::Review::Equipment(Box::new(
                        reviews::EquipmentConfirmation::new_local(id, name, profiles, alternatives),
                    )));
                }
            } else {
                let (changes, _) = self.changes(definitions);
                match changes.review_equipment(snapshot, id) {
                    Ok(review) => {
                        let previous = self
                            .previous_repairs
                            .get(&id)
                            .map(Vec::as_slice)
                            .unwrap_or_default();
                        self.review = Some(reviews::Review::Equipment(Box::new(
                            reviews::EquipmentConfirmation::new_saved(review, name, previous),
                        )));
                    }
                    Err(error) => self.review_error = Some((key, error.to_string())),
                }
            }
        }
    }
}
