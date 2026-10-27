//! Reusable equipment and reviewed, atomic removal of profile defaults.

use super::operating::{
    self, Frame, Navigation, Screen, configuration, equipment_label, profile, profile_label,
};
use crate::operating::editor::EquipmentEditor;
use crate::session::settings::{SettingsAction, SettingsRequest};
use clacks_client_store::settings::SettingsSnapshot;
use clacks_model::ids::EquipmentConfigurationId;
use clacks_sync::client::ClientRecord;
use eframe::egui;
use tokio::sync::mpsc;

use crate::operating::equipment::EquipmentForm;
use clacks_client_store::settings::ProfileEquipmentReplacement;

/// Select an explicit saved configuration association; an unavailable ID remains visible.
pub(super) fn default_selector(
    ui: &mut egui::Ui,
    id: &str,
    selected: &mut Option<EquipmentConfigurationId>,
    records: &[ClientRecord],
    known_deletions: &std::collections::BTreeSet<clacks_sync::record::RecordId>,
) {
    let label = ui.label("Associated default equipment configuration");
    let text = selected
        .map(|id| {
            records
                .iter()
                .filter(|r| !known_deletions.contains(&r.key.id))
                .filter_map(configuration)
                .find(|e| e.id == id)
                .map(equipment_label)
                .unwrap_or_else(|| format!("Unavailable {id} — replace or choose None"))
        })
        .unwrap_or_else(|| "None (loading keeps current equipment)".into());
    egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "None");
            for e in records
                .iter()
                .filter(|r| !known_deletions.contains(&r.key.id))
                .filter_map(configuration)
            {
                ui.selectable_value(selected, Some(e.id), equipment_label(e));
            }
        })
        .response
        .labelled_by(label.id);
}

/// Render configuration editing and deletion without changing active operating facts.
pub(super) fn show(
    ui: &mut egui::Ui,
    frame: &mut Frame,
    snapshot: &SettingsSnapshot,
    sender: &mpsc::Sender<SettingsRequest>,
    navigation: &mut Navigation,
    known_deletions: &std::collections::BTreeSet<clacks_sync::record::RecordId>,
) {
    match &mut frame.screen {
        Screen::Equipment => {
            if ui.button("New configuration").clicked() {
                navigation.next = Some(Screen::EquipmentEdit(Box::new(EquipmentEditor::new(
                    None,
                    EquipmentForm::default(),
                ))));
            }
            let mut preview = None;
            for record in snapshot
                .equipment
                .iter()
                .filter(|r| !known_deletions.contains(&r.key.id))
            {
                if let Some(equipment) = configuration(record) {
                    ui.push_id(equipment.id, |ui| {
                        ui.label(equipment_label(equipment));
                        if ui.button("Edit configuration").clicked() {
                            navigation.next =
                                Some(Screen::EquipmentEdit(Box::new(EquipmentEditor::new(
                                    Some(record.clone()),
                                    EquipmentForm::from_values(Some(&equipment.values)),
                                ))));
                        }
                        if ui.button("Review deletion").clicked() {
                            preview = Some(equipment.id);
                        }
                    });
                }
            }
            if let Some(id) = preview {
                frame.send(sender, SettingsAction::PreviewEquipmentDeletion { id });
            }
            if ui.button("Back").clicked() {
                navigation.back = true;
            }
        }
        Screen::EquipmentEdit(editor) => {
            ui.heading("Equipment configuration editor");
            ui.label("Save changes only this reusable configuration. It does not select, load or apply it.");
            let old_id = editor.id;
            egui::ComboBox::from_id_salt("equipment.target")
                .selected_text(if editor.expected.is_some() {
                    format!("Update {}", editor.id)
                } else {
                    "Create new configuration".into()
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(editor.expected.is_none(), "Create new configuration")
                        .clicked()
                    {
                        editor.id = Default::default();
                        editor.expected = None;
                    }
                    for record in snapshot
                        .equipment
                        .iter()
                        .filter(|r| !known_deletions.contains(&r.key.id))
                    {
                        if let Some(e) = configuration(record)
                            && ui
                                .selectable_label(
                                    editor.id == e.id,
                                    format!("Update {}", equipment_label(e)),
                                )
                                .clicked()
                        {
                            editor.id = e.id;
                            editor.expected = Some(record.clone());
                        }
                    }
                });
            if editor.id != old_id {
                frame.stale = None;
                frame.validation.invalidate();
            }
            let baseline = editor.expected.as_ref().and_then(configuration);
            let edited = operating::fields::text_changed(
                &editor.name,
                baseline.map(|value| value.name.as_str()),
            );
            if operating::fields::text(
                ui,
                "equipment.name",
                "Configuration name",
                &mut editor.name,
                &frame.errors,
                edited,
            ) {
                frame.validation.edited(ui.input(|input| input.time));
            }
            let baseline = EquipmentForm::from_values(baseline.map(|value| &value.values));
            if operating::fields::equipment(
                ui,
                "equipment",
                &mut editor.equipment,
                &frame.errors,
                Some(&baseline),
            ) {
                frame.validation.edited(ui.input(|input| input.time));
            }
            if crate::ui::actions::primary(
                ui,
                "Save configuration",
                frame.validation.can_submit(&frame.errors),
            )
            .clicked()
            {
                match editor.build() {
                    Ok(next) => {
                        let expected = editor.expected.clone();
                        frame.send(sender, SettingsAction::SaveEquipment { expected, next });
                    }
                    Err(errors) => frame.errors = errors,
                }
            }
            if ui.button("Cancel configuration").clicked() {
                navigation.back = true;
            }
        }
        Screen::DeleteEquipment(review) => {
            ui.heading("Delete equipment configuration");
            if let Some(e) = configuration(&review.preview.equipment) {
                ui.label(equipment_label(e));
            }
            ui.label("Choose a replacement or remove the default from every affected profile. Copied operating values are preserved.");
            for record in &review.preview.referring_profiles {
                if let Some(p) = profile(record) {
                    ui.label(profile_label(p));
                    let choice = review.choices.get(&p.id).copied();
                    let label = match choice {
                        None => "Choose treatment".into(),
                        Some(None) => "Remove default".into(),
                        Some(Some(id)) => review
                            .preview
                            .replacement_equipment
                            .iter()
                            .filter_map(configuration)
                            .find(|e| e.id == id)
                            .map(equipment_label)
                            .unwrap_or_else(|| format!("Unavailable {id}")),
                    };
                    egui::ComboBox::from_id_salt(("delete.repair", p.id))
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(choice == Some(None), "Remove default")
                                .clicked()
                            {
                                review.choices.insert(p.id, None);
                            }
                            for e in review
                                .preview
                                .replacement_equipment
                                .iter()
                                .filter_map(configuration)
                            {
                                if ui
                                    .selectable_label(
                                        choice == Some(Some(e.id)),
                                        equipment_label(e),
                                    )
                                    .clicked()
                                {
                                    review.choices.insert(p.id, Some(e.id));
                                }
                            }
                        });
                }
            }
            let complete = review
                .preview
                .referring_profiles
                .iter()
                .filter_map(profile)
                .all(|p| review.choices.contains_key(&p.id));
            if ui
                .add_enabled(
                    complete,
                    egui::Button::new("Delete configuration and repair profiles"),
                )
                .clicked()
            {
                let expected = review.preview.clone();
                let replacements = review
                    .choices
                    .iter()
                    .map(|(&profile, &equipment)| ProfileEquipmentReplacement {
                        profile,
                        equipment,
                    })
                    .collect();
                frame.send(
                    sender,
                    SettingsAction::DeleteEquipment {
                        expected,
                        replacements,
                    },
                );
            }
            if ui.button("Cancel deletion").clicked() {
                navigation.back = true;
            }
        }
        _ => {}
    }
}
