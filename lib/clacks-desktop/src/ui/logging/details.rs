//! Additional native contact facts without expandable or duplicated quick fields.

use super::super::operating::fields;
use super::entry::text;
use crate::{logging::form::ContactForm, operating::FieldError};
use clacks_model::group::DefinitionSet;
use eframe::egui;

/// Contact side, stable widget scope and its selected display-only directory facts.
pub(in crate::ui) struct DetailPresentation<'a> {
    /// Whether these fields describe our operating station.
    pub own: bool,
    /// Scope distinguishing independently edited contacts.
    pub owner: &'a str,
    /// Already selected hints for contacted-station details.
    pub fallback: &'a super::directory::DirectoryFallback,
}

/// Show one station side's additional facts, retaining the same parent contact form.
///
/// Use a stable `owner` that distinguishes independently edited contact forms.
pub(in crate::ui) fn show(
    ui: &mut egui::Ui,
    form: &mut ContactForm,
    baseline: &ContactForm,
    definitions: &DefinitionSet,
    errors: &[FieldError],
    presentation: DetailPresentation<'_>,
) -> super::entry::EntryAction {
    let DetailPresentation {
        own,
        owner,
        fallback,
    } = presentation;
    let prefix = if own { "Our " } else { "Contact " };
    let relative_errors: Vec<_> = errors
        .iter()
        .map(|error| FieldError {
            field: error
                .field
                .strip_prefix(prefix)
                .unwrap_or(&error.field)
                .to_owned(),
            message: error.message.clone(),
        })
        .collect();
    let focused = ui.memory(|m| m.focused());
    let enter = !ui.ctx().any_popup_open() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let result = ui.push_id(owner, |ui| {
        ui.spacing_mut().text_edit_width = f32::INFINITY;
        let mut changed = false;
        if own {
            changed |= fields::station(
                ui,
                &format!("{owner}.own"),
                &mut form.own,
                definitions,
                &relative_errors,
                Some(&baseline.own),
            );
            ui.strong("Operator and equipment");
            changed |= text(
                ui,
                "contact.operator",
                "Operator callsign",
                &mut form.operator,
                &baseline.operator,
                errors,
                true,
            )
            .changed();
            changed |= text(
                ui,
                "contact.operator-name",
                "Operator name",
                &mut form.operator_name,
                &baseline.operator_name,
                errors,
                false,
            )
            .changed();
            changed |= text(
                ui,
                "contact.power",
                "Power · W",
                &mut form.power,
                &baseline.power,
                errors,
                true,
            )
            .changed();
            changed |= fields::equipment(
                ui,
                &format!("{owner}.equipment"),
                &mut form.equipment,
                errors,
                Some(&baseline.equipment),
            );
        } else {
            changed |= fields::station_details(
                ui,
                &format!("{owner}.dx"),
                &mut form.dx,
                definitions,
                &relative_errors,
                Some(&baseline.dx),
                fallback.station(),
            );
            fallback.details(ui);
            changed |= text(
                ui,
                "contact.dx-operator",
                "Contact operator callsign",
                &mut form.dx_operator,
                &baseline.dx_operator,
                errors,
                true,
            )
            .changed();
            changed |= text(
                ui,
                "contact.mode-parent",
                "Mode parent",
                &mut form.mode_parent,
                &baseline.mode_parent,
                errors,
                true,
            )
            .changed();
            ui.strong("Receive override");
            ui.small("Leave both blank to use the primary pair in both directions.");
            if text(
                ui,
                "contact.rx-frequency",
                "Receive MHz",
                &mut form.receive.frequency,
                &baseline.receive.frequency,
                errors,
                true,
            )
            .changed()
            {
                form.receive.frequency_changed();
                changed = true;
            }
            if text(
                ui,
                "contact.rx-band",
                "Receive band",
                &mut form.receive.band,
                &baseline.receive.band,
                errors,
                true,
            )
            .changed()
            {
                form.receive.band_changed();
                changed = true;
            }
        }
        changed
    });
    let save = enter
        && focused.is_some_and(|id| {
            egui::TextEdit::load_state(ui.ctx(), id).is_some()
                && ui.ctx().read_response(id).is_some_and(|response| {
                    response.lost_focus() && result.response.rect.contains(response.rect.center())
                })
        });
    super::entry::EntryAction {
        edited: result.inner,
        save,
    }
}
