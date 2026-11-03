//! Compact two-pane entry above the current contact list.

#[cfg(test)]
mod tests;

use super::{logging::LoggingUi, theme};
use crate::session::settings::SettingsRequest;
use clacks_client_store::settings::SettingsSnapshot;
use eframe::egui;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Workspace-local pane dimensions, separate from recorded operating facts.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct PaneSizes {
    /// Preferred upper logging-area height.
    height: f32,
    /// Preferred quick-entry width.
    width: f32,
}

impl Default for PaneSizes {
    fn default() -> Self {
        Self {
            height: 350.0,
            width: 610.0,
        }
    }
}

/// Adjacent tools share one pane without changing the unfinished QSO.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    /// Prior contacts for the current call.
    #[default]
    WorkedBefore,
    /// Additional contacted-station facts.
    Contact,
    /// Contact-specific own operating settings.
    OurSettings,
}

/// Real logging-region owner, retaining one entry across pane navigation.
#[derive(Default)]
pub(super) struct Workbench {
    /// Shared new-contact entry and its adjacent details.
    pub entry: LoggingUi,
    /// Actual bounded local contact list and saved editor.
    contacts: super::contacts::ContactsUi,
    /// One prior-contact data owner, independent of the history expression.
    worked_before: super::worked_before::WorkedBeforeUi,
    /// Last committed settings view used for actual mutation invalidation.
    previous: Option<SettingsSnapshot>,
    /// Local presentation preferences.
    sizes: PaneSizes,
    /// Current adjacent tool, independent of an optional native window.
    tab: DetailTab,
    /// Apply reset dimensions for one frame, then resume ordinary resizing.
    reset: bool,
}

impl Workbench {
    /// Route only matching contact/key results to their original form.
    pub(super) fn lookup_update(&mut self, update: crate::directory::lookup::LookupUpdate) {
        if update.changed {
            self.entry.changed = true;
        }
        self.entry.lookup_update(&update);
        self.contacts.lookup_update(&update);
    }

    /// Open an existing referring contact without changing entry inputs or history scope.
    pub fn open_contact(
        &mut self,
        id: clacks_model::ids::QsoId,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        self.contacts.open(id, requests);
    }

    /// Invalidate both contact consumers after a committed data or definition change.
    pub fn invalidate_contacts(
        &mut self,
        view: clacks_client_store::logbooks::selection::SelectedView,
    ) {
        self.contacts.refresh(view);
        self.worked_before.refresh();
    }
    /// Finish local query persistence before the runtime stops admitting work.
    pub fn flush_query(
        &mut self,
        context: &egui::Context,
        requests: &mpsc::Sender<SettingsRequest>,
    ) -> bool {
        self.contacts.flush_query(context, requests)
    }

    /// Surface a failed final query write while shutdown is still recoverable.
    pub fn query_save_error(&self) -> Option<&str> {
        self.contacts.filter.error.as_deref()
    }

    /// Retry the pending local preference write without discarding the edit.
    pub fn retry_query_save(&mut self) {
        self.contacts.filter.retry();
    }
    /// Restore only pane sizes, never unfinished contact input.
    pub fn restore(&mut self, storage: Option<&dyn eframe::Storage>) {
        self.contacts.restore(storage);
        self.worked_before.restore(storage);
        self.sizes = storage
            .and_then(|s| eframe::get_value(s, "clacks.workbench.panes.v1"))
            .unwrap_or_default();
        if !self.sizes.height.is_finite() || !self.sizes.width.is_finite() {
            self.sizes = PaneSizes::default();
        }
    }

    /// Persist local dimensions through the application storage lifecycle.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        self.contacts.save(storage);
        self.worked_before.save(storage);
        eframe::set_value(storage, "clacks.workbench.panes.v1", &self.sizes);
    }

    /// Expose layout commands in the application menu without using logging space.
    pub fn menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            ui.menu_button("Layout", |ui| {
                if ui.button("Reset workbench layout").clicked() {
                    self.sizes = PaneSizes::default();
                    self.reset = true;
                    ui.close();
                }
            });
        });
    }

    /// Draw the production logging panes using current committed defaults.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &SettingsSnapshot,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        self.entry.refresh(snapshot);
        let contact_changed = std::mem::take(&mut self.entry.changed);
        if self.previous.as_ref() != Some(snapshot) || contact_changed {
            self.invalidate_contacts(snapshot.logbooks.selected_view);
            self.previous = Some(snapshot.clone());
        }
        let available = ui.available_size();
        let height = self
            .sizes
            .height
            .clamp(240.0_f32.min(available.y), (available.y - 90.0).max(240.0));
        let mut top = egui::Panel::top("workbench.entry")
            .frame(egui::Frame::NONE)
            .resizable(true)
            .default_size(height)
            .min_size(220.0)
            .max_size((available.y - 90.0).max(240.0));
        if self.reset {
            top = top.exact_size(height);
        }
        let top = top.show(ui, |ui| {
            let width = self
                .sizes
                .width
                .clamp(240.0, (ui.available_width() - 280.0).max(550.0));
            let mut left = egui::Panel::left("workbench.quick")
                .frame(theme::section_frame(ui))
                .resizable(true)
                .default_size(width)
                .min_size(550.0)
                .max_size((ui.available_width() - 280.0).max(550.0));
            if self.reset {
                left = left.exact_size(width);
            }
            let left = left.show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("entry.quick.scroll")
                    .show(ui, |ui| self.entry.show(ui, requests));
            });
            self.sizes.width = left.response.rect.width();
            self.worked_before.update(
                ui.ctx(),
                self.entry.contact_call(),
                snapshot.logbooks.selected_view,
                crate::logging::time::now(),
                requests,
            );
            egui::CentralPanel::default()
                .frame(theme::section_frame(ui))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.tab,
                            DetailTab::WorkedBefore,
                            "Worked Before",
                        );
                        ui.selectable_value(&mut self.tab, DetailTab::Contact, "Contact details");
                        ui.selectable_value(&mut self.tab, DetailTab::OurSettings, "Our settings");
                    });
                    ui.separator();
                    match self.tab {
                        DetailTab::WorkedBefore => {
                            if let Some(id) = self.worked_before.show(ui) {
                                self.contacts.open(id, requests);
                            }
                        }
                        DetailTab::Contact => self.entry.details(ui, requests, false),
                        DetailTab::OurSettings => self.entry.details(ui, requests, true),
                    }
                });
        });
        self.sizes.height = top.response.rect.height();
        self.reset = false;
        self.contacts
            .show(ui, snapshot, requests, self.entry.definitions());
        if let Some(id) = self.worked_before.show_window(ui.ctx()) {
            self.contacts.open(id, requests);
        }
        if std::mem::take(&mut self.contacts.changed) {
            self.invalidate_contacts(snapshot.logbooks.selected_view);
        }
    }
}
