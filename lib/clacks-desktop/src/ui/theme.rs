//! Shared logical sizing and semantic state colors for the native UI.

use super::preferences::{Appearance, AppearancePreferences, FontChoice};
use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle, Theme};

/// State colors kept separate from focus and selection styling owned by egui.
pub(super) struct Palette {
    /// Background for values that have not been applied.
    pub edited_fill: Color32,
    /// Persistent edge marker accompanying the edited-value text.
    pub edited_edge: Color32,
    /// Error border and message color, independent of edited state.
    pub error: Color32,
}

/// Select semantic colors for a concrete framework appearance.
pub(super) fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            edited_fill: Color32::from_rgb(57, 51, 38),
            edited_edge: Color32::from_rgb(230, 189, 106),
            error: Color32::from_rgb(255, 178, 183),
        },
        Theme::Light => Palette {
            edited_fill: Color32::from_rgb(255, 247, 223),
            edited_edge: Color32::from_rgb(148, 97, 0),
            error: Color32::from_rgb(165, 43, 50),
        },
    }
}

/// Install both appearance styles and let egui combine theme and native scaling.
pub(super) fn install(context: &egui::Context, preferences: &AppearancePreferences) {
    context.set_theme(match preferences.appearance {
        Appearance::System => egui::ThemePreference::System,
        Appearance::Light => egui::ThemePreference::Light,
        Appearance::Dark => egui::ThemePreference::Dark,
    });
    context.set_zoom_factor(preferences.zoom);
    let ui_family = match preferences.ui_font {
        FontChoice::Default => FontFamily::Proportional,
        FontChoice::Installed { .. } => FontFamily::Name("clacks.ui".into()),
    };
    let radio_family = match preferences.radio_font {
        FontChoice::Default => FontFamily::Monospace,
        FontChoice::Installed { .. } => FontFamily::Name("clacks.radio".into()),
    };

    for theme in [Theme::Light, Theme::Dark] {
        context.style_mut_of(theme, |style| {
            style
                .text_styles
                .insert(TextStyle::Body, FontId::new(13.0, ui_family.clone()));
            style
                .text_styles
                .insert(TextStyle::Button, FontId::new(13.0, ui_family.clone()));
            style
                .text_styles
                .insert(TextStyle::Small, FontId::new(12.0, ui_family.clone()));
            style
                .text_styles
                .insert(TextStyle::Heading, FontId::new(15.0, ui_family.clone()));
            style.text_styles.insert(
                TextStyle::Monospace,
                FontId::new(13.0, radio_family.clone()),
            );
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(8.0, 4.0);
            style.spacing.interact_size.y = 30.0;
            style.spacing.window_margin = egui::Margin::same(12);
            style.visuals.error_fg_color = palette(theme).error;
        });
    }
}
