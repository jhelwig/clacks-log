//! Narrow source and operator dialogs, using the existing pending operating context.

use super::super::*;

/// A source/operator choice with its own captured operating basis.
pub(in crate::ui) enum QuickEdit {
    /// Copy station values and the profile's optional equipment default.
    Profile(Box<OperatingEditor>),
    /// Replace only copied equipment and its selected baseline.
    Equipment(Box<OperatingEditor>),
    /// Replace only independently optional operator information.
    Operator(Box<OperatingEditor>),
}

impl QuickEdit {
    /// Inspect the captured operating form without advancing its save basis.
    pub fn editor(&self) -> &OperatingEditor {
        match self {
            Self::Profile(editor) | Self::Equipment(editor) | Self::Operator(editor) => editor,
        }
    }

    /// Adjust this choice while retaining the original stale-save basis.
    pub fn editor_mut(&mut self) -> &mut OperatingEditor {
        match self {
            Self::Profile(editor) | Self::Equipment(editor) | Self::Operator(editor) => editor,
        }
    }
}

/// Present explicit source/operator choices; saving reusable records is a separate workflow.
pub(in crate::ui::operating) fn show(
    ui: &mut egui::Ui,
    frame: &mut Frame,
    snapshot: &SettingsSnapshot,
    definitions: &DefinitionSet,
    sender: &mpsc::Sender<SettingsRequest>,
    navigation: &mut Navigation,
    known_deletions: &BTreeSet<RecordId>,
) {
    let Screen::Quick(edit) = &mut frame.screen else {
        return;
    };
    match edit {
        QuickEdit::Profile(editor) => {
            ui.label("Apply copies this station and its associated default equipment. Operator and power stay unchanged.");
            for saved in snapshot
                .profiles
                .iter()
                .filter(|record| !known_deletions.contains(&record.key.id))
                .filter_map(profile)
            {
                if ui
                    .button(format!("Load {}", profile_label(saved)))
                    .clicked()
                {
                    let default = saved.default_equipment.and_then(|id| {
                        snapshot
                            .equipment
                            .iter()
                            .filter(|record| !known_deletions.contains(&record.key.id))
                            .filter_map(configuration)
                            .find(|equipment| equipment.id == id)
                    });
                    match editor.stage_profile(saved, default) {
                        Ok(()) => {
                            frame.validation.invalidate();
                            frame.notice = None;
                        }
                        Err(error) => {
                            frame.notice = Some(format!(
                                "Could not load profile: {error:#}. No station or equipment values were changed."
                            ))
                        }
                    }
                }
            }
            if let Some(selected) = &editor.selected_profile {
                ui.strong(format!("Chosen profile: {}", profile_label(selected)));
                ui.label(match selected.default_equipment {
                    Some(_) => format!(
                        "Associated equipment to load: {}",
                        editor
                            .selected_equipment
                            .as_ref()
                            .map(equipment_label)
                            .unwrap_or_else(|| "Unavailable".into())
                    ),
                    None => "No default equipment: keep current equipment and selection.".into(),
                });
                if snapshot
                    .profiles
                    .iter()
                    .filter_map(profile)
                    .any(|saved| saved.id == selected.id && saved != selected)
                {
                    ui.label("Saved profile changed. Preview remains as copied; load it explicitly to refresh.");
                }
            }
            let preview = editor.build(definitions).ok();
            if let Some(preview) = preview {
                fields::active_summary(ui, &preview);
            }
        }
        QuickEdit::Equipment(editor) => {
            ui.heading("Choose equipment configuration");
            ui.label("Apply replaces equipment only. Station, operator and power stay unchanged.");
            for saved in snapshot
                .equipment
                .iter()
                .filter(|record| !known_deletions.contains(&record.key.id))
                .filter_map(configuration)
            {
                if ui
                    .button(format!("Load {}", equipment_label(saved)))
                    .clicked()
                {
                    editor.stage_equipment(saved);
                    frame.validation.invalidate();
                }
            }
            if let Some(selected) = &editor.selected_equipment {
                ui.strong(format!("Chosen equipment: {}", equipment_label(selected)));
                if snapshot
                    .equipment
                    .iter()
                    .filter_map(configuration)
                    .any(|saved| saved.id == selected.id && saved != selected)
                {
                    ui.label("Saved equipment changed. Preview remains as copied; load it explicitly to refresh.");
                }
            }
            let mut preview = editor.equipment.clone();
            ui.add_enabled_ui(false, |ui| {
                fields::equipment(ui, "quick.equipment.preview", &mut preview, &[], None)
            });
        }
        QuickEdit::Operator(editor) => {
            ui.heading("Change operator");
            ui.label("Station profile and equipment selections stay unchanged. Either operator value may be blank.");
            let original = editor
                .expected
                .current
                .as_ref()
                .and_then(|value| value.operator.as_ref());
            let edited_call = fields::callsign_changed(
                &editor.operator_callsign,
                original
                    .and_then(|operator| operator.callsign.as_ref())
                    .map(|call| call.as_str()),
            );
            let edited_name = fields::text_changed(
                &editor.operator_name,
                original.and_then(|operator| operator.recorded_name.as_deref()),
            );
            let mut changed = fields::radio_text(
                ui,
                "quick.operator.call",
                "Operator callsign",
                &mut editor.operator_callsign,
                &frame.errors,
                edited_call,
            );
            changed |= fields::text(
                ui,
                "quick.operator.name",
                "Operator name",
                &mut editor.operator_name,
                &frame.errors,
                edited_name,
            );
            if changed {
                frame.validation.edited(ui.input(|input| input.time));
            }
        }
    }
    ui.separator();
    if crate::ui::actions::primary(ui, "Apply", frame.validation.can_submit(&frame.errors))
        .clicked()
    {
        match edit.editor().build(definitions) {
            Ok(next) => {
                let expected = edit.editor().expected.clone();
                frame.send(sender, SettingsAction::Apply { expected, next });
            }
            Err(errors) => frame.errors = errors,
        }
    }
    if ui.button("Cancel").clicked() {
        navigation.back = true;
    }
}
