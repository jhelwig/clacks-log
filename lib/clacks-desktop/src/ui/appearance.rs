//! Appearance preview and explicit commit/discard without changing operating data.

use super::{
    fonts::FontsUi,
    preferences::{Appearance, AppearancePreferences, STORAGE_KEY},
    theme,
};
use crate::session::SessionCommand;
use eframe::egui;
use tokio::sync::mpsc;

/// Pending presentation choices, isolated from the persisted preferences.
pub(super) struct AppearanceEditor {
    /// Values currently shown in the preview.
    pub pending: AppearancePreferences,
}

impl AppearanceEditor {
    /// Start a preview using the last committed presentation choices.
    pub fn new(preferences: &AppearancePreferences) -> Self {
        Self {
            pending: preferences.clone(),
        }
    }
}

/// An explicit outcome from the presentation editor.
pub(super) enum AppearanceAction {
    /// Commit the valid preview as local preferences.
    Apply,
    /// Restore the committed appearance and discard the preview.
    Cancel,
}

/// Application-owned preference state shared by every view in this client.
#[derive(Default)]
pub(super) struct AppearanceUi {
    /// Actual native-family catalog and role requests, independent of domain forms.
    fonts: FontsUi,
    /// Only these values reach the framework save hook.
    pub committed: AppearancePreferences,
    /// Optional preview whose changes remain unapplied.
    pub editor: Option<AppearanceEditor>,
    /// Actionable preference-loading feedback, separate from workspace failures.
    pub notice: Option<String>,
}

impl AppearanceUi {
    /// Advance font work for the current preview or committed preferences.
    pub fn update_fonts(
        &mut self,
        context: &egui::Context,
        commands: &mpsc::Sender<SessionCommand>,
    ) {
        let preferences = self
            .editor
            .as_ref()
            .map(|editor| &editor.pending)
            .unwrap_or(&self.committed);
        self.fonts
            .update(context, preferences, commands, self.editor.is_some());
    }

    /// Restore validated local preferences or keep usable defaults with a notice.
    pub fn restore(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(storage) = storage else {
            return Self::default();
        };
        let Some(_) = storage.get_string(STORAGE_KEY) else {
            return Self::default();
        };
        match eframe::get_value::<AppearancePreferences>(storage, STORAGE_KEY) {
            Some(preferences) if preferences.validate().is_ok() => Self {
                committed: preferences,
                ..Self::default()
            },
            _ => Self {
                notice: Some("Saved appearance could not be loaded. Defaults are in use; review Appearance settings.".into()),
                ..Self::default()
            },
        }
    }

    /// Save only committed presentation choices through the native framework.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.committed);
    }

    /// Open the local appearance editor without replacing an existing preview.
    pub fn open(&mut self) {
        self.editor
            .get_or_insert_with(|| AppearanceEditor::new(&self.committed));
    }

    /// Complete a user action and install the corresponding valid appearance.
    pub fn finish(&mut self, context: &egui::Context, action: AppearanceAction) {
        if matches!(action, AppearanceAction::Apply) {
            let Some(editor) = &self.editor else {
                return;
            };
            if editor.pending.validate().is_err() {
                return;
            }
            self.committed = editor.pending.clone();
            self.notice = None;
        }
        if self.editor.take().is_some() {
            context.request_repaint();
        }
        theme::install(context, &self.committed);
    }

    /// Whether the preview holds a change for the shared Settings draft.
    pub fn changed(&self) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|editor| editor.pending != self.committed)
    }

    /// Draw the Appearance section and install valid previews immediately.
    pub fn show_section(&mut self, ui: &mut egui::Ui) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let before = editor.pending.clone();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut editor.pending.appearance, Appearance::System, "System");
            ui.selectable_value(&mut editor.pending.appearance, Appearance::Light, "Light");
            ui.selectable_value(&mut editor.pending.appearance, Appearance::Dark, "Dark");
        });
        let scale_label = ui.label("UI scale");
        ui.add(
            egui::Slider::new(&mut editor.pending.zoom, 0.75..=2.0)
                .step_by(0.05)
                .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
        )
        .labelled_by(scale_label.id);
        if ui.button("Reset scale to 100%").clicked() {
            editor.pending.zoom = 1.0;
        }
        self.fonts.show(ui, &mut editor.pending);
        for error in editor.pending.validate().err().unwrap_or_default() {
            super::feedback::problem(ui, &error.message);
        }
        if let Some(notice) = &self.notice {
            super::feedback::problem(ui, notice);
        }
        if editor.pending != before && editor.pending.validate().is_ok() {
            theme::install(ui.ctx(), &editor.pending);
        }
    }
}
