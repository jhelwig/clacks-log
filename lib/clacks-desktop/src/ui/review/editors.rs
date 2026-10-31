//! Concrete resolved-value controls shared with normal contact editing.

use crate::{
    operating::FieldError,
    review::input::FieldInput,
    ui::{
        fields::{self, FieldPresentation},
        operating::fields as operating_fields,
    },
};
use clacks_model::group::DefinitionSet;
use eframe::egui;

/// Edit one explicitly chosen input using the existing labeled field treatment.
pub(super) fn show(
    ui: &mut egui::Ui,
    label: &str,
    input: &mut FieldInput,
    definitions: &DefinitionSet,
    settings: &clacks_client_store::settings::SettingsSnapshot,
    errors: &[FieldError],
    edited: bool,
) -> bool {
    match input {
        FieldInput::Text { value } => text(ui, label, value, errors, edited),
        FieldInput::Position { form } => {
            let mut changed = false;
            let mut grid = form.grid.clone();
            if text(ui, "Grid locator", &mut grid, errors, edited) {
                form.edit_grid(grid);
                changed = true;
            }
            let mut latitude = form.latitude.clone();
            let mut longitude = form.longitude.clone();
            let coordinates_changed = text(ui, "Latitude", &mut latitude, errors, edited)
                | text(ui, "Longitude", &mut longitude, errors, edited);
            if coordinates_changed {
                form.edit_coordinates(latitude, longitude);
                changed = true;
            }
            ui.weak(match form.kind {
                crate::operating::position::PositionKind::Grid => {
                    "Grid area; coordinates show its center"
                }
                crate::operating::position::PositionKind::Coordinates => "Explicit coordinates",
            });
            if form.kind == crate::operating::position::PositionKind::Coordinates {
                let old = form.length;
                egui::ComboBox::from_label("Locator precision")
                    .selected_text(form.length.get().to_string())
                    .show_ui(ui, |ui| {
                        for size in [4, 6, 8, 10, 12] {
                            if let Ok(length) = clacks_model::position::GridLength::new(size) {
                                ui.selectable_value(&mut form.length, length, size.to_string());
                            }
                        }
                    });
                changed |= old != form.length;
                if old != form.length {
                    form.edit_precision(form.length);
                }
            }
            changed
        }
        FieldInput::Radio { form } => {
            let frequency = text(ui, "MHz", &mut form.frequency, errors, edited);
            if frequency {
                form.frequency_changed();
            }
            let band = text(ui, "Band", &mut form.band, errors, edited);
            if band {
                form.band_changed();
            }
            frequency || band
        }
        FieldInput::Mode { label, parent } => {
            text(ui, "Mode", label, errors, edited)
                | text(ui, "Parent mode", parent, errors, edited)
        }
        FieldInput::References { rows } => {
            operating_fields::references(ui, "review.references", rows, definitions, errors)
        }
        FieldInput::Equipment { selected } => {
            let before = *selected;
            crate::ui::equipment::default_selector(
                ui,
                "review.default_equipment",
                selected,
                &settings.equipment,
                &Default::default(),
            );
            before != *selected
        }
        FieldInput::Policy { value } => {
            let before = *value;
            crate::ui::logbooks::editor::policy(ui, label, value, edited);
            before != *value
        }
        FieldInput::Membership { .. } => false,
    }
}

/// Present the shared text-field styles with native keyboard and validation behavior.
fn text(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    errors: &[FieldError],
    edited: bool,
) -> bool {
    fields::text(
        ui,
        ui.scope_id().with(label),
        label,
        value,
        FieldPresentation {
            edited,
            read_only: !ui.is_enabled(),
            errors,
            validation_label: None,
            radio_data: !["Comment", "name", "Equipment", "Antenna", "notes"]
                .iter()
                .any(|word| label.contains(word)),
        },
    )
    .changed()
}
