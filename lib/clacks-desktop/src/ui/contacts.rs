//! Production saved-contact list and independent edit/delete workflows.

mod deletion;
mod editor;
mod membership;
mod query;
mod responses;
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
    logbooks::selection::SelectedView,
    read::contacts::{ContactPage, ContactPageQuery},
    save::SaveOutcome,
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
    /// Last successfully read bounded page.
    page: Option<ContactPage>,
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
    /// Logical focused contact for keyboard navigation.
    focused: Option<QsoId>,
    /// Keyboard target awaiting native focus after scrolling into the rendered rows.
    pending_focus: Option<QsoId>,
    /// Known mutations or view changes require another bounded read.
    dirty: bool,
    /// Persistent operation feedback, independent of editor dismissal.
    notice: Option<String>,
    /// Distinguish quiet committed feedback from an unresolved error.
    problem: bool,
    /// Current record returned by a stale save, available only for explicit reload.
    current_record: Option<ClientRecord>,
    /// Whether the first page request has been admitted.
    initialized: bool,
    /// Committed contact change awaiting workbench-wide read invalidation.
    pub(super) changed: bool,
}

impl ContactsUi {
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
            self.membership_view_changed();
            self.query.view = view;
            self.selected.clear();
            self.query.show_selected = false;
            self.query.selected_ids.clear();
            self.focused = None;
            self.pending_focus = None;
            self.page = None;
        }
        self.query.offset = 0;
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
        self.poll_membership();
        if self.membership.is_none() {
            egui::Popup::close_id(ui.ctx(), egui::Id::unique("contacts.membership"));
        }
        let history = egui::CentralPanel::default().frame(theme::section_frame(ui));
        history.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Contact history");
                if ui.small_button("Refresh contacts").clicked() {
                    self.refresh(self.query.view);
                }
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
                }
                if ui.small_button("Clear selection").clicked() {
                    self.selected.clear();
                    if std::mem::take(&mut self.query.show_selected) {
                        self.refresh(self.query.view);
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
            self.membership_feedback(ui);
            self.filter.show(ui, snapshot, definitions);
            let action = table::show(
                ui,
                self.page.as_ref(),
                &mut self.preferences,
                &mut self.selected,
                &mut self.focused,
                &mut self.pending_focus,
                None,
            );
            if self.query.order != self.preferences.order {
                self.query.order = self.preferences.order;
                self.refresh(self.query.view);
            }
            if let Some(id) = action.open {
                self.request(ContactAction::Load { id }, requests);
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        self.query.offset > 0 && self.page_request.is_none(),
                        egui::Button::new("Previous"),
                    )
                    .clicked()
                {
                    self.query.offset = self.query.offset.saturating_sub(self.query.limit);
                    self.dirty = true;
                }
                if ui
                    .add_enabled(
                        self.page.as_ref().is_some_and(|p| p.has_more)
                            && self.page_request.is_none(),
                        egui::Button::new("Next"),
                    )
                    .clicked()
                {
                    self.query.offset = self.query.offset.saturating_add(self.query.limit);
                    self.dirty = true;
                }
                let matching = self.page.as_ref().map_or(0, |p| p.matching_total);
                let selected = self.page.as_ref().map_or(0, |p| p.selected_total);
                let hidden = self.page.as_ref().map_or(0, |p| p.selected_outside_filter);
                ui.small(format!(
                    "Page {} · {matching} matching · {selected} selected · {hidden} outside filter",
                    self.query.offset / self.query.limit + 1
                ));
                if self.page_request.is_some() {
                    ui.small("Loading…");
                }
            });
        });
        self.filter.advance(ui.ctx(), requests, false);
        self.update_query();
        self.read_page(requests);
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
        }
        if self.query.selected_ids != self.selected {
            self.query.selected_ids = self.selected.clone();
            self.dirty = true;
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
        if self.filter.desired.is_none()
            || !(self.dirty || !self.initialized)
            || self.page_request.is_some()
        {
            return;
        }
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
                receiver,
            });
            self.dirty = false;
            self.initialized = true;
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
