//! Compact contact fields in the agreed row order.

use super::super::fields::{self, FieldPresentation};
use crate::{logging::form::ContactForm, operating::FieldError};
use eframe::egui;

/// User actions from quick controls, independent of validation and persistence.
#[derive(Default)]
pub(in crate::ui) struct EntryAction {
    /// At least one input changed.
    pub edited: bool,
    /// Enter left a single-line field and requested submission.
    pub save: bool,
}

/// Render a stable text field and retain its native response for keyboard handling.
pub(in crate::ui) fn text(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    value: &mut String,
    baseline: &str,
    errors: &[FieldError],
    radio: bool,
) -> egui::Response {
    text_with_fallback(
        ui,
        QuickField {
            id: key,
            label,
            value,
            baseline,
        },
        errors,
        radio,
        None,
    )
}

/// Display optional directory text without inserting it into the recorded input.
fn text_with_fallback(
    ui: &mut egui::Ui,
    field: QuickField<'_>,
    errors: &[FieldError],
    radio: bool,
    fallback: Option<fields::FieldFallback<'_>>,
) -> egui::Response {
    let QuickField {
        id: key,
        label,
        value,
        baseline,
    } = field;
    let edited = value != baseline;
    fields::text_with_fallback(
        ui,
        ui.make_persistent_id(key),
        label,
        value,
        FieldPresentation {
            edited,
            read_only: !ui.is_enabled(),
            errors,
            validation_label: Some(match key {
                "entry.call" => "Contact Station callsign",
                "entry.frequency" | "entry.band" => "Frequency / Band",
                "entry.grid" => "Contact Position",
                "contact.power" => "Power",
                _ => label,
            }),
            radio_data: radio,
        },
        fallback,
    )
}

/// Named input metadata for compact rows.
struct QuickField<'a> {
    /// Stable scoped input identity.
    id: &'a str,
    /// Visible input caption.
    label: &'a str,
    /// Current editable text.
    value: &'a mut String,
    /// Original text used for the unapplied-change highlight.
    baseline: &'a str,
}

/// Show quick entry without duplicating additional fields or timestamps in details.
pub(in crate::ui) fn show(
    ui: &mut egui::Ui,
    form: &mut ContactForm,
    baseline: &ContactForm,
    errors: &[FieldError],
    focus: bool,
    live_controls: bool,
    fallback: &super::directory::DirectoryFallback,
) -> EntryAction {
    let mut action = EntryAction::default();
    ui.columns(2, |columns| {
        for (column, field) in columns.iter_mut().zip([
            QuickField {
                id: "entry.call",
                label: "Callsign",
                value: &mut form.dx.callsign,
                baseline: baseline.dx.callsign.as_str(),
            },
            QuickField {
                id: "entry.name",
                label: "Name",
                value: &mut form.dx_name,
                baseline: baseline.dx_name.as_str(),
            },
        ]) {
            let had_focus = column.memory(|m| m.has_focus(column.make_persistent_id(field.id)));
            let response = text_with_fallback(
                column,
                QuickField {
                    id: field.id,
                    label: field.label,
                    value: field.value,
                    baseline: field.baseline,
                },
                errors,
                field.id == "entry.call",
                if field.id == "entry.name" {
                    fallback.name()
                } else {
                    None
                },
            );
            if focus && field.id == "entry.call" {
                response.request_focus();
            }
            collect(column, &response, &mut action, had_focus);
        }
    });
    let mut frequency_changed = false;
    let mut band_changed = false;
    let mut grid_changed = false;
    ui.columns(4, |columns| {
        for (column, field) in columns.iter_mut().zip([
            QuickField {
                id: "entry.frequency",
                label: "MHz",
                value: &mut form.primary.frequency,
                baseline: baseline.primary.frequency.as_str(),
            },
            QuickField {
                id: "entry.band",
                label: "Band",
                value: &mut form.primary.band,
                baseline: baseline.primary.band.as_str(),
            },
            QuickField {
                id: "entry.mode",
                label: "Mode",
                value: &mut form.mode,
                baseline: baseline.mode.as_str(),
            },
            QuickField {
                id: "entry.grid",
                label: "DX grid",
                value: &mut form.dx.position.grid,
                baseline: baseline.dx.position.grid.as_str(),
            },
        ]) {
            let had_focus = column.memory(|m| m.has_focus(column.make_persistent_id(field.id)));
            let response = text_with_fallback(
                column,
                QuickField {
                    id: field.id,
                    label: field.label,
                    value: field.value,
                    baseline: field.baseline,
                },
                errors,
                true,
                if field.id == "entry.grid" {
                    fallback.station().map(|station| fields::FieldFallback {
                        value: &station.station.position.grid,
                        source: station.source,
                    })
                } else {
                    None
                },
            );
            if response.changed() {
                match field.id {
                    "entry.frequency" => frequency_changed = true,
                    "entry.band" => band_changed = true,
                    "entry.grid" => grid_changed = true,
                    _ => {}
                }
            }
            collect(column, &response, &mut action, had_focus);
        }
    });
    if frequency_changed {
        form.primary.frequency_changed();
    }
    if band_changed {
        form.primary.band_changed();
    }
    if grid_changed {
        form.dx.edit_grid(form.dx.position.grid.clone());
    }
    let clock_width = ((ui.available_width() - 144.0) / 2.0).max(160.0);
    ui.horizontal(|ui| {
        for field in [
            QuickField {
                id: "entry.sent",
                label: "Sent",
                value: &mut form.sent,
                baseline: &baseline.sent,
            },
            QuickField {
                id: "entry.received",
                label: "Received",
                value: &mut form.received,
                baseline: &baseline.received,
            },
        ] {
            ui.allocate_ui_with_layout(
                egui::vec2(60.0, 48.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let had_focus = ui.memory(|m| m.has_focus(ui.make_persistent_id(field.id)));
                    let response = text(
                        ui,
                        field.id,
                        field.label,
                        field.value,
                        field.baseline,
                        errors,
                        true,
                    );
                    collect(ui, &response, &mut action, had_focus);
                },
            );
        }
        for (label, value, original) in [
            ("Start · UTC", &mut form.start, &baseline.start),
            ("End · UTC", &mut form.end, &baseline.end),
        ] {
            ui.allocate_ui_with_layout(
                egui::vec2(clock_width, 48.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let result =
                        super::timestamps::show(ui, label, value, original, errors, live_controls);
                    action.edited |= result.edited;
                    action.save |= result.save;
                },
            );
        }
    });
    let label = ui.small("Comment");
    let response = ui
        .add(
            egui::TextEdit::multiline(&mut form.comment)
                .desired_rows(1)
                .desired_width(f32::INFINITY)
                .id(ui.make_persistent_id("entry.comment")),
        )
        .labelled_by(label.id);
    action.edited |= response.changed();
    action
}

/// Interpret Enter only for the single-line field that actually relinquished focus.
fn collect(ui: &egui::Ui, response: &egui::Response, action: &mut EntryAction, had_focus: bool) {
    action.edited |= response.changed();
    action.save |=
        had_focus && response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
}
