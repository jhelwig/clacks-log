//! Production saved-contact list and independent edit/delete workflows.

mod deletion;
mod editor;
mod preferences;
mod responses;
mod table;
#[cfg(test)]
mod table_tests;
#[cfg(test)]
mod tests;

use super::{feedback, theme};
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
    /// Runtime acknowledgment.
    receiver: oneshot::Receiver<SettingsResponse>,
}

/// Independent contact history and saved-edit state.
#[derive(Default)]
pub(super) struct ContactsUi {
    query: ContactPageQuery,
    /// Last successfully read bounded page.
    page: Option<ContactPage>,
    /// Originating page request used to reject obsolete results.
    page_request: Option<PageRequest>,
    /// One saved-contact operation whose acknowledgment remains owned here.
    operation: Option<oneshot::Receiver<SettingsResponse>>,

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
}

impl ContactsUi {
    /// Restore this table's presentation independently of appearance and pane sizes.
    pub fn restore(&mut self, storage: Option<&dyn eframe::Storage>) {
        self.preferences = preferences::ContactTablePreferences::restore(storage);
        self.query.order = self.preferences.order;
    }

    /// Persist presentation without persisting forms or selection.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        self.preferences.save(storage);
    }

    /// Invalidate actual view-dependent results after a committed change.
    pub fn refresh(&mut self, view: SelectedView) {
        if self.query.view != view {
            self.query.view = view;
            self.selected.clear();
            self.query.selected = None;
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
        self.poll();
        if self.query.view != snapshot.logbooks.selected_view {
            self.refresh(snapshot.logbooks.selected_view);
        }
        if (self.dirty || !self.initialized) && self.page_request.is_none() {
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
                self.page_request = Some(PageRequest { query, receiver });
                self.dirty = false;
                self.initialized = true;
            }
        }
        let history = egui::CentralPanel::default().frame(theme::section_frame(ui));
        history.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Contact history");
                if ui.small_button("Refresh contacts").clicked() {
                    self.refresh(self.query.view);
                }
                let selected_view = self.query.selected.is_some();
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
                    self.query.selected = if selected_view {
                        None
                    } else {
                        Some(self.selected.clone())
                    };
                    self.refresh(self.query.view);
                }
                if ui.small_button("Clear selection").clicked() {
                    self.selected.clear();
                    if self.query.selected.take().is_some() {
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
            let action = table::show(
                ui,
                self.page.as_ref(),
                &mut self.preferences,
                &mut self.selected,
                &mut self.focused,
                &mut self.pending_focus,
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
                let hidden = self
                    .selected
                    .iter()
                    .filter(|id| {
                        self.page
                            .as_ref()
                            .is_none_or(|page| !page.rows.iter().any(|row| &row.qso.id == *id))
                    })
                    .count();
                ui.small(format!(
                    "Page {} · {} selected · {} not on this page",
                    self.query.offset / self.query.limit + 1,
                    self.selected.len(),
                    hidden
                ));
                if self.page_request.is_some() {
                    ui.small("Loading…");
                }
            });
        });
        self.show_editor(ui.ctx(), definitions, requests);
        self.show_deletion(ui.ctx(), requests);
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

    /// Render the saved editor, retaining its expected record until an explicit reload.
    fn show_editor(
        &mut self,
        context: &egui::Context,
        definitions: &DefinitionSet,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let busy = self.operation.is_some();
        let mut save = None;
        let mut delete = false;
        let mut cancel = false;
        let mut reload = false;
        let result = super::dialog::show(
            context,
            egui::Id::unique("contacts.editor"),
            "Edit contact",
            |ui| {
                let now = ui.input(|i| i.time);
                if editor.validation.due(now) {
                    editor.errors = editor
                        .id()
                        .map_err(|e| vec![crate::operating::issue("QSO", e)])
                        .and_then(|id| editor.form.build(id, definitions))
                        .err()
                        .unwrap_or_default();
                    editor.validation.validated();
                }
                editor.validation.request_repaint(ui.ctx(), now);
                let mut enter = false;
                ui.add_enabled_ui(!busy, |ui| {
                    let quick = super::logging::entry::show(
                        ui,
                        &mut editor.form,
                        &editor.baseline,
                        &editor.errors,
                        false,
                        false,
                    );
                    enter = quick.save;
                    let mut changed = quick.edited;
                    ui.strong("Contact details");
                    let detail = super::logging::details::show(
                        ui,
                        &mut editor.form,
                        &editor.baseline,
                        definitions,
                        &editor.errors,
                        false,
                        "saved.contact",
                    );
                    changed |= detail.edited;
                    enter |= detail.save;
                    ui.strong("Our operating settings");
                    let detail = super::logging::details::show(
                        ui,
                        &mut editor.form,
                        &editor.baseline,
                        definitions,
                        &editor.errors,
                        true,
                        "saved.contact",
                    );
                    changed |= detail.edited;
                    enter |= detail.save;
                    if changed {
                        editor.validation.edited(now);
                    }
                });
                for error in &editor.errors {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("{}: {}", error.field, error.message),
                    );
                }
                if let Some(notice) = &self.notice {
                    if self.problem {
                        feedback::problem(ui, notice);
                    } else {
                        ui.small(notice);
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    cancel = ui.button("Cancel").clicked();
                    let clicked = ui
                        .add_enabled(
                            !busy && editor.validation.can_submit(&editor.errors),
                            egui::Button::new("Save changes"),
                        )
                        .clicked();
                    if !busy && (clicked || enter) {
                        match editor
                            .id()
                            .map_err(|e| vec![crate::operating::issue("QSO", e)])
                            .and_then(|id| editor.form.build(id, definitions))
                        {
                            Ok(next) => {
                                save = Some(ContactAction::Save {
                                    expected: Some(Box::new(editor.expected.clone())),
                                    next: Box::new(next),
                                })
                            }
                            Err(errors) => editor.errors = errors,
                        }
                    }
                    delete = ui
                        .add_enabled(!busy, egui::Button::new("Delete QSO…"))
                        .clicked();
                    if self.current_record.is_some() {
                        reload = ui
                            .add_enabled(!busy, egui::Button::new("Reload current contact"))
                            .clicked();
                    }
                    if busy {
                        ui.label("Saving…");
                    }
                });
            },
        );
        if delete {
            self.deletion = Some(deletion::DeletionConfirmation {
                expected: editor.expected.clone(),
                first_frame: true,
            });
        }
        if reload && let Some(record) = self.current_record.take() {
            match editor::ContactEditor::from_record(record) {
                Ok(value) => self.editor = Some(value),
                Err(error) => self.notice = Some(error.to_string()),
            }
        }
        if cancel || result.cancel_requested {
            self.editor = None;
            self.deletion = None;
        } else if let Some(action) = save {
            self.request(action, requests);
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
