//! Primary-action button styling for Save and Apply.

use super::theme;
use eframe::egui;

/// Present a primary Save/Apply action with the active theme's contrasting accent.
///
/// Pass current validation/admission readiness as `enabled` and use the returned
/// native response to submit the owning workflow. Hover, focus and disabled
/// interaction remain managed by egui.
pub(super) fn primary(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let colors = theme::palette(ui.ctx().theme());
    ui.scope(|ui| {
        ui.visuals_mut().override_text_color = Some(colors.on_accent);
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.weak_bg_fill = colors.accent;
        widgets.inactive.bg_fill = colors.accent;
        widgets.inactive.bg_stroke = egui::Stroke::new(1.0, colors.accent);
        widgets.inactive.fg_stroke = egui::Stroke::new(1.0, colors.on_accent);
        widgets.hovered = egui::style::WidgetVisuals {
            weak_bg_fill: colors.accent.gamma_multiply(1.08),
            bg_fill: colors.accent.gamma_multiply(1.08),
            bg_stroke: egui::Stroke::new(1.0, colors.text),
            ..widgets.inactive
        };
        widgets.active = egui::style::WidgetVisuals {
            weak_bg_fill: colors.accent.gamma_multiply(0.9),
            bg_fill: colors.accent.gamma_multiply(0.9),
            bg_stroke: egui::Stroke::new(2.0, colors.on_accent),
            ..widgets.inactive
        };
        ui.add_enabled(enabled, egui::Button::new(label))
    })
    .inner
}

#[cfg(test)]
mod tests;
