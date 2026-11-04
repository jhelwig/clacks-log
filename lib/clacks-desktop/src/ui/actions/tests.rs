//! Native inspection found the shared text override defeating primary-action contrast.
use super::*;
use crate::ui::preferences::{Appearance, AppearancePreferences};
use egui_kittest::Harness;

#[test]
fn primary_action_uses_its_contrasting_text_in_both_themes() {
    for appearance in [Appearance::Light, Appearance::Dark] {
        let harness = Harness::new_ui(move |ui| {
            theme::install(
                ui.ctx(),
                &AppearancePreferences {
                    appearance,
                    ..Default::default()
                },
            );
            primary(ui, "Apply", true);
        });
        let colors = theme::palette(harness.ctx.theme());
        let text = harness
            .output()
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == "Apply" => Some(text),
                _ => None,
            })
            .expect("painted primary action");
        let recorded = text.galley.job.sections.first().unwrap().format.color;
        let actual = text.override_text_color.unwrap_or_else(|| {
            if recorded == egui::Color32::PLACEHOLDER {
                text.fallback_color
            } else {
                recorded
            }
        });
        assert_eq!(
            actual, colors.on_accent,
            "the general text override must not defeat primary-action contrast"
        );
    }
}
