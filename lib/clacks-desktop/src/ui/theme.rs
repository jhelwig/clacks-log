//! Shared colors, typography and control styling for the desktop UI.

use super::preferences::{Appearance, AppearancePreferences, FontChoice};
use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle, Theme};

/// Workbench surfaces and semantic colors shared by native components.
pub(super) struct Palette {
    /// Canvas between logging tools and other content sections.
    pub workspace: Color32,
    /// Application chrome, dialogs and ordinary section bodies.
    pub surface: Color32,
    /// Raised section headers and the operating strip.
    pub header: Color32,
    /// Recessed editable fields and value selectors.
    pub input: Color32,
    /// Restrained outlines separating controls and sections.
    pub border: Color32,
    /// Primary readable content.
    pub text: Color32,
    /// Secondary labels and supporting information.
    pub muted: Color32,
    /// Primary actions, focus and links.
    pub accent: Color32,
    /// Text on an accent-filled action.
    pub on_accent: Color32,
    /// Hovered controls and open selectors.
    pub hover: Color32,
    /// Selected rows and text-selection backgrounds.
    pub selection: Color32,
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
            workspace: Color32::from_rgb(29, 34, 40),
            surface: Color32::from_rgb(37, 42, 48),
            header: Color32::from_rgb(42, 47, 53),
            input: Color32::from_rgb(29, 34, 38),
            border: Color32::from_rgb(73, 82, 91),
            text: Color32::from_rgb(228, 234, 239),
            muted: Color32::from_rgb(181, 192, 203),
            accent: Color32::from_rgb(173, 214, 231),
            on_accent: Color32::from_rgb(22, 36, 44),
            hover: Color32::from_rgb(49, 59, 68),
            selection: Color32::from_rgb(48, 73, 86),
            edited_fill: Color32::from_rgb(57, 51, 38),
            edited_edge: Color32::from_rgb(230, 189, 106),
            error: Color32::from_rgb(255, 178, 183),
        },
        Theme::Light => Palette {
            workspace: Color32::from_rgb(240, 241, 245),
            surface: Color32::WHITE,
            header: Color32::from_rgb(240, 241, 245),
            input: Color32::from_rgb(245, 246, 248),
            border: Color32::from_rgb(203, 209, 216),
            text: Color32::from_rgb(30, 39, 47),
            muted: Color32::from_rgb(82, 97, 108),
            accent: Color32::from_rgb(50, 103, 129),
            on_accent: Color32::WHITE,
            hover: Color32::from_rgb(232, 238, 243),
            selection: Color32::from_rgb(219, 233, 240),
            edited_fill: Color32::from_rgb(255, 247, 223),
            edited_edge: Color32::from_rgb(148, 97, 0),
            error: Color32::from_rgb(165, 43, 50),
        },
    }
}

/// Give a real tool section the active appearance's restrained boundary and surface.
pub(super) fn section_frame(ui: &egui::Ui) -> egui::Frame {
    let colors = palette(ui.ctx().theme());
    egui::Frame::group(ui.style())
        .fill(colors.surface)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(2)
        .inner_margin(8)
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
                .insert(TextStyle::Body, FontId::new(12.0, ui_family.clone()));
            style
                .text_styles
                .insert(TextStyle::Button, FontId::new(12.0, ui_family.clone()));
            style
                .text_styles
                .insert(TextStyle::Small, FontId::new(11.0, ui_family.clone()));
            style
                .text_styles
                .insert(TextStyle::Heading, FontId::new(14.0, ui_family.clone()));
            style.text_styles.insert(
                TextStyle::Monospace,
                FontId::new(13.0, radio_family.clone()),
            );
            style.spacing.item_spacing = egui::vec2(6.0, 4.0);
            style.spacing.button_padding = egui::vec2(6.0, 3.0);
            style.spacing.interact_size.y = 24.0;
            style.spacing.window_margin = egui::Margin::same(12);
            let colors = palette(theme);
            let visuals = &mut style.visuals;
            visuals.override_text_color = Some(colors.text);
            visuals.weak_text_color = Some(colors.muted);
            visuals.panel_fill = colors.surface;
            visuals.window_fill = colors.surface;
            visuals.window_stroke = egui::Stroke::new(1.0, colors.border);
            visuals.window_corner_radius = egui::CornerRadius::same(4);
            visuals.menu_corner_radius = egui::CornerRadius::same(2);
            visuals.extreme_bg_color = colors.input;
            visuals.text_edit_bg_color = Some(colors.input);
            visuals.code_bg_color = colors.input;
            visuals.faint_bg_color = colors.header;
            visuals.hyperlink_color = colors.accent;
            visuals.error_fg_color = colors.error;
            visuals.warn_fg_color = colors.edited_edge;
            visuals.selection.bg_fill = colors.selection;
            visuals.selection.stroke = egui::Stroke::new(1.0, colors.accent);
            let resting = egui::style::WidgetVisuals {
                bg_fill: colors.input,
                weak_bg_fill: colors.input,
                bg_stroke: egui::Stroke::new(1.0, colors.border),
                corner_radius: egui::CornerRadius::same(2),
                fg_stroke: egui::Stroke::new(1.0, colors.text),
                expansion: 0.0,
            };
            visuals.widgets.inactive = resting;
            visuals.widgets.noninteractive = egui::style::WidgetVisuals {
                bg_fill: colors.surface,
                weak_bg_fill: colors.surface,
                ..resting
            };
            visuals.widgets.hovered = egui::style::WidgetVisuals {
                bg_fill: colors.hover,
                weak_bg_fill: colors.hover,
                bg_stroke: egui::Stroke::new(1.0, colors.accent),
                ..resting
            };
            visuals.widgets.active = egui::style::WidgetVisuals {
                bg_fill: colors.header,
                weak_bg_fill: colors.header,
                bg_stroke: egui::Stroke::new(1.0, colors.accent),
                ..resting
            };
            visuals.widgets.open = visuals.widgets.hovered;
        });
    }
}
