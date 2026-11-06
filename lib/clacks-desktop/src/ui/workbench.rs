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
    /// Last native minimum sent; viewport commands otherwise request a repaint
    /// every frame even when their value is unchanged.
    window_minimum: Option<(egui::Vec2, f32)>,
}

impl Workbench {
    /// Admit the only directory lookup action against actual selected identities.
    pub fn start_directory_lookup(&mut self, requests: &mpsc::Sender<SettingsRequest>) {
        self.contacts.start_directory_lookup(requests);
    }
    /// Current selection and run lifecycle determine menu availability.
    pub fn can_lookup_directory(&self) -> bool {
        self.contacts.can_lookup_directory()
    }
    /// Poll non-modal bulk work independently from painting contact tables.
    pub fn poll_bulk_lookup(&mut self) {
        self.contacts.poll_bulk_lookup();
    }
    /// A logbook change clears selection before queued native commands can consume it.
    pub fn reconcile_history_view(
        &mut self,
        view: clacks_client_store::logbooks::selection::SelectedView,
    ) {
        self.contacts.reconcile_view(view);
    }

    /// Route the shared Edit command to the actual history-selection owner.
    pub fn select_all_contacts(&mut self, requests: &mpsc::Sender<SettingsRequest>) {
        self.contacts.select_all(requests);
    }

    /// True only when the history table owns focus and its applied query is ready.
    pub fn history_has_focus(&self, context: &egui::Context) -> bool {
        self.contacts.history_has_focus(context)
    }

    /// Keep native selection admission in step with pending read acknowledgments.
    pub fn poll_selection(&mut self) {
        self.contacts.poll_selection();
    }

    /// Selection consumers cannot act on an incomplete Select all request.
    pub fn selection_pending(&self) -> bool {
        self.contacts.selection_pending()
    }

    /// Apply the WS callsign to the shared entry observed by lookup and history.
    pub(super) fn incoming_call(&mut self, call: &str) {
        self.entry.incoming_call(call);
    }

    /// Advance directory lookup demand for the entry and saved-contact editor.
    pub(super) fn advance_lookup(
        &mut self,
        context: &egui::Context,
        lookup: Option<&crate::directory::lookup::LookupClient>,
    ) {
        self.entry.advance_lookup(context, lookup);
        self.contacts.advance_lookup(context, lookup);
    }

    /// Measure the composed tabs using the same typography and icon allocation
    /// as the visible strip, without admitting input or painting another strip.
    fn detail_minimum_width(&mut self, ui: &mut egui::Ui) -> f32 {
        let mut sizing = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("workbench.tabs.measure")
                .sizing_pass()
                .invisible(),
        );
        self.tabs(&mut sizing);
        sizing.min_rect().width().ceil()
    }

    /// Join the active tab to its pane and report a separate-window request.
    fn tabs(&mut self, ui: &mut egui::Ui) -> bool {
        let colors = theme::palette(ui.ctx().theme());
        let mut active = None;
        let mut open_window = false;
        let strip = ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.spacing_mut().button_padding.y = 6.0;
            let content_height = ui.text_style_height(&egui::TextStyle::Button).max(12.0);
            let tab_height = content_height + 2.0 * ui.spacing().button_padding.y + 2.0;
            for (tab, label) in [
                (DetailTab::WorkedBefore, "Worked Before"),
                (DetailTab::Contact, "Contact details"),
                (DetailTab::OurSettings, "Our settings"),
            ] {
                let selected = self.tab == tab;
                let icon_id = egui::IdSalt::new("worked_before.popout");
                let mut button = egui::Button::new(label)
                    .min_size(egui::vec2(0.0, tab_height))
                    .selected(selected)
                    .frame(true)
                    .frame_when_inactive(true)
                    .fill(if selected {
                        colors.surface
                    } else {
                        colors.header
                    })
                    .corner_radius(egui::CornerRadius {
                        nw: 3,
                        ne: 3,
                        sw: 0,
                        se: 0,
                    });
                if tab == DetailTab::WorkedBefore {
                    button = button.right_text(egui::Atom::custom(
                        icon_id,
                        egui::vec2(20.0, content_height),
                    ));
                }
                let atoms = button.atom_ui(ui);
                if let Some(rect) = atoms.rect(icon_id) {
                    let rect = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, atoms.response.rect.center().y),
                        egui::vec2(rect.width(), atoms.response.rect.height() - 2.0),
                    );
                    let mut icon_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                    icon_ui.spacing_mut().button_padding = egui::Vec2::ZERO;
                    let icon = icon_ui.place(
                        rect,
                        egui::Button::new(egui::Atom::paint(egui::vec2(12.0, 12.0), |ui, args| {
                            let r = args.rect;
                            let stroke = egui::Stroke::new(1.5, ui.visuals().text_color());
                            ui.painter().add(egui::Shape::line(
                                vec![
                                    egui::pos2(r.center().x, r.top()),
                                    r.left_top(),
                                    r.left_bottom(),
                                    r.right_bottom(),
                                    egui::pos2(r.right(), r.center().y),
                                ],
                                stroke,
                            ));
                            ui.painter()
                                .line_segment([r.center(), r.right_top()], stroke);
                            ui.painter().add(egui::Shape::line(
                                vec![
                                    egui::pos2(r.center().x + 1.0, r.top()),
                                    r.right_top(),
                                    egui::pos2(r.right(), r.center().y - 1.0),
                                ],
                                stroke,
                            ));
                        }))
                        .frame(true)
                        .frame_when_inactive(false),
                    );
                    icon.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            "Open Worked Before in new window",
                        )
                    });
                    open_window |= icon.clicked();
                    icon.on_hover_text("Open Worked Before in new window");
                }
                let response = atoms.response;
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::Tab, ui.is_enabled(), selected, label)
                });
                if selected {
                    active = Some(response.rect);
                }
                if response.clicked() {
                    self.tab = tab;
                }
            }
        });
        let bottom = strip.response.rect.bottom();
        ui.painter().hline(
            ui.max_rect().x_range(),
            bottom,
            egui::Stroke::new(1.0, colors.border),
        );
        if let Some(active) = active {
            ui.painter().hline(
                active.left() + 1.0..=active.right() - 1.0,
                bottom,
                egui::Stroke::new(2.0, colors.surface),
            );
        }
        open_window
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

    /// Whether a saved-contact modal currently owns workbench input.
    pub fn has_modal(&self) -> bool {
        self.contacts.has_modal()
    }

    /// Restore pane sizes without changing contact input.
    pub fn reset_layout(&mut self) {
        self.sizes = PaneSizes::default();
        self.reset = true;
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
        let detail_minimum = self.detail_minimum_width(ui);
        let window_overhead = ui
            .ctx()
            .input(|input| input.viewport().inner_rect)
            .map_or(0.0, |rect| (rect.width() - available.x).max(0.0));
        let history_minimum = self.contacts.minimum_height(ui);
        let vertical_overhead = ui
            .ctx()
            .input(|input| input.viewport().inner_rect)
            .map_or(0.0, |rect| (rect.height() - available.y).max(0.0));
        let window_minimum = egui::vec2(
            550.0 + detail_minimum + window_overhead,
            220.0 + history_minimum + vertical_overhead,
        );
        let minimum_state = (window_minimum, ui.ctx().pixels_per_point());
        if self.window_minimum != Some(minimum_state) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::MinInnerSize(window_minimum));
            self.window_minimum = Some(minimum_state);
        }
        let height = self.sizes.height.clamp(
            220.0_f32.min(available.y),
            (available.y - history_minimum).max(220.0),
        );
        let mut top = egui::Panel::top("workbench.entry")
            .frame(egui::Frame::NONE)
            .resizable(true)
            .default_size(height)
            .min_size(220.0)
            .max_size((available.y - history_minimum).max(220.0));
        if self.reset {
            top = top.exact_size(height);
        }
        let top = top.show(ui, |ui| {
            let width = self
                .sizes
                .width
                .clamp(550.0, (ui.available_width() - detail_minimum).max(550.0));
            let mut left = egui::Panel::left("workbench.quick")
                .frame(theme::pane_frame(ui))
                .resizable(true)
                .default_size(width)
                .min_size(550.0)
                .max_size((ui.available_width() - detail_minimum).max(550.0));
            if self.reset {
                left = left.exact_size(width);
            }
            let left = left.show(ui, |ui| {
                self.entry.show(ui, requests);
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
                .frame(theme::pane_frame(ui))
                .show(ui, |ui| {
                    let spacing = ui.spacing().item_spacing;
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if self.tabs(ui) {
                        self.worked_before.open_window();
                    }
                    ui.spacing_mut().item_spacing = spacing;
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
