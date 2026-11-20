//! Source selection controls used by sync and import comparisons.

use crate::review::{fields::FieldId, input::FieldInput};
use clacks_client_store::settings::SettingsSnapshot;
use clacks_model::{group::DefinitionSet, ids::TagId};
use eframe::egui;
use std::collections::BTreeMap;

/// One selectable source for a field in the shared comparison controls.
pub(in crate::ui) struct SourceChoice<'a> {
    pub label: &'a str,
    pub selected: bool,
    /// The display input, or the text explaining why the choice is disabled.
    pub input: Result<&'a FieldInput, &'a str>,
}

impl SourceChoice<'_> {
    /// Render the source value or its unavailable-state message. Return true only
    /// when the user selects an available source.
    pub fn show(
        self,
        ui: &mut egui::Ui,
        field: FieldId,
        tags: &BTreeMap<TagId, String>,
        definitions: &DefinitionSet,
        settings: &SettingsSnapshot,
    ) -> bool {
        let clicked = ui
            .add_enabled(
                self.input.is_ok(),
                egui::Button::selectable(self.selected, self.label),
            )
            .clicked();
        match self.input {
            Ok(input) => super::value::show(ui, field, input, tags, definitions, settings),
            Err(reason) => {
                ui.weak(reason);
            }
        }
        clicked
    }
}

/// Return the source chosen with an enabled bulk-selection button.
pub(in crate::ui) fn choose_all<T>(ui: &mut egui::Ui, options: [(T, &str, bool); 2]) -> Option<T> {
    let mut chosen = None;
    for (source, label, available) in options {
        if ui
            .add_enabled(available, egui::Button::new(label))
            .clicked()
        {
            chosen = Some(source);
        }
    }
    chosen
}
