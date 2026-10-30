//! Reusable station profiles, separate from pending and applied operating values.

use super::{
    equipment,
    operating::{
        self, Frame, Navigation, Screen, configuration, equipment_label, profile, profile_label,
    },
};
use crate::operating::editor::ProfileEditor;
use crate::session::settings::{SettingsAction, SettingsRequest};
use clacks_client_store::settings::SettingsSnapshot;
use clacks_model::group::DefinitionSet;
use eframe::egui;
use tokio::sync::mpsc;

use crate::operating::station::StationForm;

/// Present the profile list/editor without loading saved values into operation.
pub(super) fn show(
    ui: &mut egui::Ui,
    frame: &mut Frame,
    snapshot: &SettingsSnapshot,
    definitions: &DefinitionSet,
    sender: &mpsc::Sender<SettingsRequest>,
    navigation: &mut Navigation,
    known_deletions: &std::collections::BTreeSet<clacks_sync::record::RecordId>,
) {
    match &mut frame.screen {
        Screen::Profiles => {
            ui.label(
                "A profile owns station values and an optional default equipment configuration.",
            );
            if ui.button("New profile").clicked() {
                navigation.next = Some(Screen::Profile(Box::new(ProfileEditor::new(
                    None,
                    StationForm::empty(),
                ))));
            }
            for record in snapshot
                .profiles
                .iter()
                .filter(|r| !known_deletions.contains(&r.key.id))
            {
                if let Some(profile) = profile(record) {
                    ui.push_id(profile.id(), |ui| {
                        ui.label(profile_label(profile));
                        ui.label(format!(
                            "Default equipment: {}",
                            profile
                                .default_equipment()
                                .map(|id| snapshot
                                    .equipment
                                    .iter()
                                    .filter_map(configuration)
                                    .find(|e| e.id() == id)
                                    .map(equipment_label)
                                    .unwrap_or_else(|| format!("Unavailable {id}")))
                                .unwrap_or_else(|| "None (loading keeps current equipment)".into())
                        ));
                        if ui.button("Edit profile").clicked() {
                            navigation.next = Some(Screen::Profile(Box::new(ProfileEditor::new(
                                Some(record.clone()),
                                StationForm::from_snapshot(profile.station()),
                            ))));
                        }
                    });
                }
            }
            if ui.button("Back").clicked() {
                navigation.back = true;
            }
        }
        Screen::Profile(editor) => {
            ui.heading("Station profile editor");
            ui.label("Save changes only this saved profile. It does not select, load or apply it.");
            // Explicitly choosing a target retains the typed station values, not that target's facts.
            let old_id = editor.id;
            egui::ComboBox::from_id_salt("profile.target")
                .selected_text(if editor.expected.is_some() {
                    format!("Update {}", editor.id)
                } else {
                    "Create new profile".into()
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(editor.expected.is_none(), "Create new profile")
                        .clicked()
                    {
                        editor.id = Default::default();
                        editor.expected = None;
                        editor.default_equipment = None;
                    }
                    for record in snapshot
                        .profiles
                        .iter()
                        .filter(|r| !known_deletions.contains(&r.key.id))
                    {
                        if let Some(p) = profile(record)
                            && ui
                                .selectable_label(
                                    editor.id == p.id(),
                                    format!("Update {}", profile_label(p)),
                                )
                                .clicked()
                        {
                            editor.id = p.id();
                            editor.expected = Some(record.clone());
                            editor.default_equipment = p.default_equipment();
                        }
                    }
                })
                .response
                .widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::ComboBox,
                        ui.is_enabled(),
                        "Profile save target",
                    )
                });
            if editor.id != old_id {
                frame.stale = None;
                frame.validation.invalidate();
            }
            let baseline = editor.expected.as_ref().and_then(profile);
            let edited =
                operating::fields::text_changed(&editor.name, baseline.map(|value| value.name()));
            if operating::fields::text(
                ui,
                "profile.name",
                "Profile name",
                &mut editor.name,
                &frame.errors,
                edited,
            ) {
                frame.validation.edited(ui.input(|input| input.time));
            }
            let previous_default = editor.default_equipment;
            equipment::default_selector(
                ui,
                "profile.default",
                &mut editor.default_equipment,
                &snapshot.equipment,
                known_deletions,
            );
            if previous_default != editor.default_equipment {
                frame.validation.invalidate();
            }
            if ui.button("Manage equipment").clicked() {
                navigation.next = Some(Screen::Equipment);
            }
            let baseline = baseline.map(|value| StationForm::from_snapshot(value.station()));
            if operating::fields::station(
                ui,
                "profile.station",
                &mut editor.station,
                definitions,
                &frame.errors,
                baseline.as_ref(),
            ) {
                frame.validation.edited(ui.input(|input| input.time));
            }
            if crate::ui::actions::primary(
                ui,
                "Save profile",
                frame.validation.can_submit(&frame.errors),
            )
            .clicked()
            {
                match editor.build(definitions) {
                    Ok(next) => {
                        let expected = editor.expected.clone();
                        frame.send(sender, SettingsAction::SaveProfile { expected, next });
                    }
                    Err(errors) => frame.errors = errors,
                }
            }
            if let Screen::Profile(editor) = &frame.screen
                && let Some(expected) = &editor.expected
            {
                let expected = expected.clone();
                if ui.button("Delete profile").clicked() {
                    frame.send(sender, SettingsAction::DeleteProfile { expected });
                }
            }
            if ui.button("Cancel profile").clicked() {
                navigation.back = true;
            }
        }
        _ => {}
    }
}
