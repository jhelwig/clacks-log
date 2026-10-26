//! Native labeled fields with independent editing, focus and validation presentation.

#[cfg(test)]
mod tests;

use super::theme;
use crate::operating::FieldError;
use eframe::egui;

/// Caller-owned facts displayed by one field, independent of its save workflow.
pub(super) struct FieldPresentation<'a> {
    /// Value differs from the corresponding unapplied/committed comparison basis.
    pub edited: bool,
    /// Display a readable value without accepting input.
    pub read_only: bool,
    /// Current form errors; matching labels are shown beside this control.
    pub errors: &'a [FieldError],
    /// Native validation key when a compound input uses a different visible caption.
    pub validation_label: Option<&'a str>,
    /// Use the configurable radio-data typeface rather than the ordinary UI font.
    pub radio_data: bool,
}

/// Present a stable native text edit and return its ordinary egui interaction response.
pub(super) fn text(
    ui: &mut egui::Ui,
    id: egui::Id,
    label: &str,
    value: &mut String,
    presentation: FieldPresentation<'_>,
) -> egui::Response {
    let colors = theme::palette(ui.ctx().theme());
    let label_id = ui
        .horizontal(|ui| {
            let label_id = ui.small(egui::RichText::new(label).color(colors.muted)).id;
            if presentation.edited {
                ui.small(egui::RichText::new("Not applied").color(colors.edited_edge));
            }
            label_id
        })
        .inner;
    let mut edit = egui::TextEdit::singleline(value)
        .id(id)
        .font(if presentation.radio_data {
            egui::TextStyle::Monospace
        } else {
            egui::TextStyle::Body
        })
        .interactive(!presentation.read_only);
    if presentation.edited {
        edit = edit.background_color(colors.edited_fill);
    }
    if presentation.read_only {
        edit = edit.text_color(ui.visuals().text_color());
    }
    let response = ui.add(edit).labelled_by(label_id);
    if presentation.edited {
        ui.painter().vline(
            response.rect.left() + 2.0,
            response.rect.top() + 3.0..=response.rect.bottom() - 3.0,
            egui::Stroke::new(2.0, colors.edited_edge),
        );
    }
    for error in presentation
        .errors
        .iter()
        .filter(|error| error.field == presentation.validation_label.unwrap_or(label))
    {
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            3,
            egui::Stroke::new(1.0, colors.error),
            egui::StrokeKind::Outside,
        );
        ui.colored_label(colors.error, egui::RichText::new(&error.message).small());
    }
    response
}

/// Present a compact label and current-value button as one wrapping unit.
///
/// Use the returned response to open the relevant choice editor. The label uses
/// secondary typography while the value retains its supplied text style.
pub(super) fn choice(
    ui: &mut egui::Ui,
    label: &str,
    value: impl Into<egui::WidgetText>,
    enabled: bool,
) -> egui::Response {
    let value = value.into();
    let caption = egui::RichText::new(label)
        .small()
        .color(theme::palette(ui.ctx().theme()).muted);
    let caption_width = egui::WidgetText::from(caption.clone())
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Small,
        )
        .size()
        .x;
    let value_width = value
        .clone()
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        )
        .size()
        .x;
    let width = caption_width
        + value_width
        + ui.spacing().item_spacing.x
        + 2.0 * ui.spacing().button_padding.x;
    ui.allocate_ui_with_layout(
        egui::vec2(width, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let label = ui.label(caption);
            ui.add_enabled(enabled, egui::Button::new(value))
                .labelled_by(label.id)
        },
    )
    .inner
}
