//! Shared prefix history for the logging pane and optional dedicated window.

mod model;
#[cfg(test)]
mod tests;
mod window;

use super::contact_table::{
    self,
    preferences::{ContactTablePreferences, TableKind},
};
use crate::session::settings::SettingsRequest;
use clacks_client_store::logbooks::selection::SelectedView;
use clacks_model::{ids::QsoId, scalar::UtcSecond};
use eframe::egui;
use std::collections::BTreeSet;
use tokio::sync::mpsc;

/// Window-local row interaction; contacts and column choices remain shared.
#[derive(Default)]
struct TableInteraction {
    /// Stable selected identities for native row presentation.
    selected: BTreeSet<QsoId>,
    /// Keyboard-focused row identity.
    focused: Option<QsoId>,
    /// Requested keyboard focus after scrolling.
    pending_focus: Option<QsoId>,
}

/// One reusable view owner; opening its window never changes the workbench mode.
pub(super) struct WorkedBeforeUi {
    /// Shared request/data state for both presentations.
    model: model::WorkedBeforeModel,
    /// Shared column choices and ordering, independent of history preferences.
    preferences: ContactTablePreferences,
    /// Dedicated native viewport visibility only.
    window_open: bool,
    /// Focus/selection in the tab's own table.
    tab: TableInteraction,
    /// Focus/selection in the dedicated window's own table.
    window: TableInteraction,
}

impl Default for WorkedBeforeUi {
    fn default() -> Self {
        Self {
            model: Default::default(),
            preferences: ContactTablePreferences::defaults(TableKind::WorkedBefore),
            window_open: false,
            tab: Default::default(),
            window: Default::default(),
        }
    }
}

impl WorkedBeforeUi {
    /// Show the additional native view while preserving the main tab and shared query.
    pub fn open_window(&mut self) {
        self.window_open = true;
    }

    /// Restore only the independent Worked Before column choices.
    pub fn restore(&mut self, storage: Option<&dyn eframe::Storage>) {
        self.preferences = ContactTablePreferences::restore(storage, TableKind::WorkedBefore);
    }

    /// Persist shared presentation without saving window-local focus or an entry draft.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        self.preferences.save(storage);
    }

    /// Follow the actual unfinished call without depending on history expressions.
    pub fn update(
        &mut self,
        context: &egui::Context,
        call: &str,
        view: SelectedView,
        now: UtcSecond,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        self.model
            .update(context, call, view, self.preferences.order, now, requests);
    }

    /// Invalidate shared rows after a committed contact, view or definition change.
    pub fn refresh(&mut self) {
        self.model.refresh();
    }

    /// Draw prior contacts directly within the logging region's tab.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<QsoId> {
        ui.push_id("worked_before.tab", |ui| {
            show_table(ui, &mut self.model, &mut self.preferences, &mut self.tab)
        })
        .inner
    }
}

/// Present shared data through a window-local native table interaction.
fn show_table(
    ui: &mut egui::Ui,
    model: &mut model::WorkedBeforeModel,
    preferences: &mut ContactTablePreferences,
    interaction: &mut TableInteraction,
) -> Option<QsoId> {
    if !model.query.call_prefix.is_empty() || model.error.is_some() {
        egui::Panel::top("prior.controls")
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                show_controls(ui, model);
            });
    }
    egui::Panel::bottom("prior.pagination")
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            show_pagination(ui, model);
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let action = contact_table::show(
                ui,
                model.page.as_ref(),
                preferences,
                &mut interaction.selected,
                &mut interaction.focused,
                &mut interaction.pending_focus,
                Some(model.day),
            );

            action.open
        })
        .inner
}

/// Render the shared heading and query feedback in either presentation.
fn show_controls(ui: &mut egui::Ui, model: &model::WorkedBeforeModel) {
    if !model.query.call_prefix.is_empty() {
        ui.strong(format!("Worked Before · {}", model.query.call_prefix));
    }
    if let Some(error) = &model.error {
        super::feedback::problem(ui, error);
    }
    if !model.query.call_prefix.is_empty() && model.page.is_none() && model.error.is_none() {
        ui.small("Loading prior contacts…");
    }
}

/// Render the shared pagination controls in either presentation.
fn show_pagination(ui: &mut egui::Ui, model: &mut model::WorkedBeforeModel) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(model.query.offset > 0, egui::Button::new("Previous"))
            .clicked()
        {
            model.next_page(false);
        }
        if ui
            .add_enabled(
                model.page.as_ref().is_some_and(|p| p.has_more),
                egui::Button::new("Next"),
            )
            .clicked()
        {
            model.next_page(true);
        }
        ui.small(format!(
            "{} prior contacts",
            model.page.as_ref().map_or(0, |p| p.matching_total)
        ));
    });
}
