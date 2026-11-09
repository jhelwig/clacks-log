//! Production saved-contact list and independent edit/delete workflows.

mod bulk;
mod deletion;
mod editor;
mod membership;
mod query;
mod responses;
mod selection;
#[cfg(test)]
mod tests;

use super::{
    contact_table::{self as table, preferences},
    feedback, theme,
};
use crate::session::settings::{
    SettingsAction, SettingsRequest, SettingsResponse,
    contacts::{ContactAction, ContactResponse},
};
use clacks_client_store::{
    logbooks::selection::SelectedView, read::contacts::ContactPageQuery, save::SaveOutcome,
    settings::SettingsSnapshot,
};
use clacks_model::{group::DefinitionSet, ids::QsoId};
use clacks_sync::client::ClientRecord;
use eframe::egui;
use std::collections::BTreeSet;
use tokio::sync::{mpsc, oneshot};

/// One query reply remains tied to its actual selection and ordering.
struct PageRequest {
    /// Submitted query, used to reject obsolete view responses.
    /// Current selected-view page and SQL ordering.
    query: ContactPageQuery,
    /// Expression text associated with this exact request.
    expression: String,
    /// First reply after invalidation replaces all prior cached pages.
    replace: bool,
    /// Runtime acknowledgment.
    receiver: oneshot::Receiver<SettingsResponse>,
}

/// Independent contact history and saved-edit state.
#[derive(Default)]
pub(super) struct ContactsUi {
    /// Shared persisted expression editor and validated desired scope.
    pub(super) filter: query::HistoryQueryUi,
    query: ContactPageQuery,
    /// Expression text bound to the current requested page inputs.
    query_text: String,
    /// Bounded viewport rows and their complete result counts.
    data: table::data::TableData,
    /// Pages currently required by the history viewport.
    needed: BTreeSet<u32>,
    /// A changed query/order requires the viewport to return to its beginning.
    reset_scroll: bool,
    /// Failed read, separate from saved-contact operation feedback; blocks automatic retry.
    read_error: Option<String>,
    /// Originating page request used to reject obsolete results.
    page_request: Option<PageRequest>,
    /// One saved-contact operation whose acknowledgment remains owned here.
    operation: Option<oneshot::Receiver<SettingsResponse>>,

    /// Selected-contact assignment retained independently of popup lifetime.
    membership: Option<membership::BulkMembership>,
    /// Independent saved-contact form, absent when dismissed.
    editor: Option<editor::ContactEditor>,
    /// Exact record awaiting individual deletion confirmation.
    deletion: Option<deletion::DeletionConfirmation>,
    /// Locally remembered table columns and sorting.
    preferences: preferences::ContactTablePreferences,
    /// Selected native contact identities, independent of row order.
    selected: BTreeSet<QsoId>,
    /// Pending whole-query selection, independent of modal editor ownership.
    selection: Option<selection::PendingSelection>,
    /// Actual focused history widget, distinct from Worked Before and entry.
    history_focus: Option<egui::Id>,
    /// Non-modal lookup consumer, preserved independently from selection/view changes.
    bulk: Option<bulk::BulkLookupUi>,
    /// Logical focused contact for keyboard navigation.
    focused: Option<QsoId>,
    /// Keyboard target awaiting native focus after scrolling into the rendered rows.
    pending_focus: Option<table::PendingFocus>,
    /// Known mutations or view changes require another bounded read.
    dirty: bool,
    /// Persistent operation feedback, independent of editor dismissal.
    notice: Option<String>,
    /// Distinguish quiet committed feedback from an unresolved error.
    problem: bool,
    /// Current record returned by a stale save, available only for explicit reload.
    current_record: Option<ClientRecord>,
    /// Committed contact change awaiting workbench-wide read invalidation.
    pub(super) changed: bool,
    /// Measured fixed controls, used by the workbench divider constraint.
    chrome_height: f32,
}

impl ContactsUi {
    /// Keep saved-edit assistance bound to the live editor and its ordinary save boundary.
    pub(super) fn advance_geography(
        &mut self,
        commands: &mpsc::Sender<crate::session::SessionCommand>,
        status: &crate::geography::GeographyStatus,
        closing: bool,
    ) {
        if let Some(editor) = &mut self.editor {
            let frozen = closing || self.operation.is_some();
            let own = editor.own_geography.advance(
                &mut editor.form.own,
                commands,
                status,
                frozen,
                "contact_editor_own",
            );
            let dx = editor.dx_geography.advance(
                &mut editor.form.dx,
                commands,
                status,
                frozen,
                "contact_editor_contacted",
            );
            if own || dx {
                editor.validation.invalidate();
            }
        }
    }
    /// Advance pending directory demand for the saved-contact editor.
    pub(super) fn advance_lookup(
        &mut self,
        context: &egui::Context,
        lookup: Option<&crate::directory::lookup::LookupClient>,
    ) {
        if let Some(editor) = &mut self.editor {
            editor
                .directory
                .observe(&editor.form.dx.callsign, tokio::time::Instant::now());
            editor
                .directory
                .advance(context, lookup, tokio::time::Instant::now());
        }
    }

    /// Minimum allocation for fixed controls and one usable table row.
    pub fn minimum_height(&self, ui: &egui::Ui) -> f32 {
        self.chrome_height + table::minimum_height(ui)
    }

    /// A saved-contact workflow reserves modal input while its load is pending.
    pub fn has_modal(&self) -> bool {
        self.editor.is_some() || self.deletion.is_some() || self.operation.is_some()
    }

    /// Open the existing saved-contact editor from either contact table.
    pub fn open(&mut self, id: QsoId, requests: &mpsc::Sender<SettingsRequest>) {
        self.request(ContactAction::Load { id }, requests);
    }
    /// Restore this table's presentation independently of appearance and pane sizes.
    pub fn restore(&mut self, storage: Option<&dyn eframe::Storage>) {
        self.preferences =
            preferences::ContactTablePreferences::restore(storage, preferences::TableKind::History);
        self.query.order = self.preferences.order;
    }

    /// Persist presentation without persisting forms or selection.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        self.preferences.save(storage);
    }

    /// Invalidate actual view-dependent results after a committed change.
    pub fn refresh(&mut self, view: SelectedView) {
        if self.query.view != view {
            self.selection = None;
            self.history_focus = None;
            self.membership_view_changed();
            self.query.view = view;
            self.selected.clear();
            self.query.show_selected = false;
            self.query.selected_ids.clear();
            self.focused = None;
            self.pending_focus = None;
            self.data.clear();
            self.needed.clear();
            self.reset_scroll = true;
        }
        self.query.offset = 0;
        self.pending_focus = None;
        self.read_error = None;
        self.dirty = true;
        if let Some(editor) = &mut self.editor {
            editor.validation.invalidate();
        }
    }

    /// Show actual bounded contact rows and poll their originating operations.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &SettingsSnapshot,
        requests: &mpsc::Sender<SettingsRequest>,
        definitions: &DefinitionSet,
    ) {
        if self.query.view != snapshot.logbooks.selected_view {
            self.refresh(snapshot.logbooks.selected_view);
        }
        self.filter.advance(ui.ctx(), requests, false);
        self.update_query();
        self.poll();
        self.poll_selection();
        self.poll_bulk_lookup();
        self.poll_membership();
        if self.membership.is_none() {
            egui::Popup::close_id(ui.ctx(), egui::Id::unique("contacts.membership"));
        }
        let history = egui::CentralPanel::default().frame(theme::pane_frame(ui));
        history.show(ui, |ui| {
            let header = egui::Panel::top("history.controls")
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong("Contact history");
                        self.membership_action(ui, requests);
                        let selected_view = self.query.show_selected;
                        if ui
                            .add_enabled(
                                !self.selected.is_empty() || selected_view,
                                egui::Button::new(if selected_view {
                                    "Show all in view"
                                } else {
                                    "Show selected"
                                }),
                            )
                            .clicked()
                        {
                            self.query.show_selected = !selected_view;
                            self.query.selected_ids = self.selected.clone();
                            self.refresh(self.query.view);
                            self.reset_scroll = true;
                        }
                        if ui.small_button("Clear selection").clicked() {
                            self.selection = None;
                            self.selected.clear();
                            if std::mem::take(&mut self.query.show_selected) {
                                self.refresh(self.query.view);
                                self.reset_scroll = true;
                            }
                        }
                    });
                    if let Some(notice) = &self.notice {
                        if self.problem {
                            feedback::problem(ui, notice);
                        } else {
                            ui.small(notice);
                        }
                    }
                    if let Some(error) = &self.read_error {
                        feedback::problem(ui, error);
                        if ui.small_button("Retry contact read").clicked() {
                            self.read_error = None;
                        }
                    }
                    self.membership_feedback(ui);
                    self.bulk_feedback(ui);
                    if self.selection_pending() {
                        ui.small("Selecting contacts…");
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(25));
                    }
                    self.filter.show(ui, snapshot, definitions);
                });
            let footer = egui::Panel::bottom("history.counts")
                .frame(egui::Frame::NONE)
                .min_size(0.0)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let matching = self.data.counts.as_ref().map_or(0, |p| p.matching_total);
                        let selected = self.data.counts.as_ref().map_or(0, |p| p.selected_total);
                        let hidden = self
                            .data
                            .counts
                            .as_ref()
                            .map_or(0, |p| p.selected_outside_filter);
                        ui.small(format!(
                            "{matching} matching · {selected} selected · {hidden} outside filter"
                        ));
                    });
                });
            let chrome_height = header.response.rect.height() + footer.response.rect.height();
            if (self.chrome_height - chrome_height).abs() > 0.5 {
                self.chrome_height = chrome_height;
                ui.ctx()
                    .request_discard("history fixed controls changed height");
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    let action = table::show(
                        ui,
                        table::TableContent {
                            data: &self.data,
                            selected_only: self.query.show_selected,
                            reset_scroll: std::mem::take(&mut self.reset_scroll),
                            loading_message: self
                                .read_error
                                .is_none()
                                .then_some("Loading contacts…"),
                        },
                        &mut self.preferences,
                        &mut self.selected,
                        &mut self.focused,
                        &mut self.pending_focus,
                        None,
                    );
                    self.needed = action.needed;
                    if let Some(owner) = action.focus_owner {
                        self.history_focus = Some(owner);
                    }
                    if action.select_all {
                        self.select_all(requests);
                    }
                    if self.query.order != self.preferences.order {
                        self.query.order = self.preferences.order;
                        self.refresh(self.query.view);
                        self.reset_scroll = true;
                    }
                    if let Some(id) = action.open {
                        self.request(ContactAction::Load { id }, requests);
                    }
                });
        });
        self.filter.advance(ui.ctx(), requests, false);
        self.update_query();
        self.read_page(requests);
        if self.page_request.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(25));
        }
        self.show_editor(ui.ctx(), definitions, requests, &snapshot.logbooks);
        self.show_deletion(ui.ctx(), requests);
    }

    /// Align request inputs before accepting replies, including selection-only changes.
    fn update_query(&mut self) {
        if let Some(filter) = &self.filter.desired
            && (self.query.filter != *filter || self.query_text != self.filter.desired_text)
        {
            self.query.filter = filter.clone();
            self.query_text = self.filter.desired_text.clone();
            self.query.offset = 0;
            self.dirty = true;
            self.reset_scroll = true;
            self.focused = None;
            self.pending_focus = None;
            self.read_error = None;
        }
        if self.query.selected_ids != self.selected {
            self.query.selected_ids = self.selected.clone();
            self.dirty = true;
            self.read_error = None;
        }
        if self.page_request.as_ref().is_some_and(|p| {
            p.query != self.query || p.expression != self.filter.desired_text || self.dirty
        }) {
            self.page_request = None;
            self.dirty = true;
        }
    }

    /// Admit a page only after the applied query has been restored successfully.
    fn read_page(&mut self, requests: &mpsc::Sender<SettingsRequest>) {
        if self.filter.desired.is_none() || self.page_request.is_some() || self.read_error.is_some()
        {
            return;
        }
        let needed = if self.reset_scroll {
            [0].into()
        } else {
            self.needed.clone()
        };
        let offset = if self.dirty {
            Some(needed.first().copied().unwrap_or(0))
        } else {
            self.data.require(&needed)
        };
        let Some(offset) = offset else { return };
        self.query.offset = offset;
        self.query.limit = table::data::PAGE_SIZE;
        let (reply, receiver) = oneshot::channel();
        let query = self.query.clone();
        if requests
            .try_send(SettingsRequest {
                action: SettingsAction::Contacts(ContactAction::Page {
                    query: query.clone(),
                }),
                reply,
            })
            .is_ok()
        {
            self.page_request = Some(PageRequest {
                query,
                expression: self.filter.desired_text.clone(),
                replace: self.dirty,
                receiver,
            });
            self.dirty = false;
        }
    }

    /// Acknowledge the latest unfinished text before session admission closes.
    pub fn flush_query(
        &mut self,
        context: &egui::Context,
        requests: &mpsc::Sender<SettingsRequest>,
    ) -> bool {
        self.filter.flush(context, requests)
    }

    /// Admit one saved-contact operation without replacing a pending acknowledgment.
    fn request(&mut self, action: ContactAction, requests: &mpsc::Sender<SettingsRequest>) {
        if self.operation.is_some() {
            return;
        }
        let (reply, receiver) = oneshot::channel();
        match requests.try_send(SettingsRequest {
            action: SettingsAction::Contacts(action),
            reply,
        }) {
            Ok(()) => {
                self.operation = Some(receiver);

                self.notice = None;
            }
            Err(error) => {
                self.notice = Some(format!("Unable to request contact operation: {error}"))
            }
        }
    }

    /// Submit only the exact record reviewed in the individual deletion prompt.
    fn show_deletion(&mut self, context: &egui::Context, requests: &mpsc::Sender<SettingsRequest>) {
        let Some(confirmation) = &mut self.deletion else {
            return;
        };
        match deletion::show(context, confirmation, self.operation.is_some()) {
            deletion::DeletionAction::Cancel => self.deletion = None,
            deletion::DeletionAction::Delete => {
                let action = ContactAction::Delete {
                    expected: Box::new(confirmation.expected.clone()),
                };
                self.request(action, requests);
            }
            deletion::DeletionAction::None => {}
        }
    }
}
