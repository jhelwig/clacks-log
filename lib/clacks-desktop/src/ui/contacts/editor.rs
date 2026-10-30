//! Independent saved-contact editing with a retained expected record.

use super::{ContactAction, ContactsUi, SettingsRequest, deletion, feedback};
use crate::logging::form::ContactForm;
use crate::operating::FieldError;
use crate::ui::{
    tag_picker::{self, MembershipPicker},
    validation::ValidationState,
};
use clacks_client_store::logbooks::LogbookSnapshot;
use clacks_model::ids::QsoId;
use clacks_model::{group::DefinitionSet, ids::TagId};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordState, RecordValue},
};
use eframe::egui;
use std::collections::BTreeSet;
use tokio::sync::mpsc;

/// Saved facts being edited, never the unfinished new-contact entry.
pub(super) struct ContactEditor {
    /// Complete record used by the store's stale-save protection.
    pub expected: ClientRecord,
    /// Editable copied historical facts.
    pub form: ContactForm,
    /// Initial form for changed-state display.
    pub baseline: ContactForm,
    /// Current debounced native validation state.
    pub validation: ValidationState,
    /// Field errors from the complete contact.
    pub errors: Vec<FieldError>,
    /// Pending popup choices and their copied opening values.
    picker: Option<EditorMembership>,
}

impl ContactEditor {
    /// Open only actual live local working contact facts.
    pub fn from_record(expected: ClientRecord) -> anyhow::Result<Self> {
        let Some(RecordState::Live(RecordValue::Qso(qso))) = expected.working_state() else {
            anyhow::bail!("This contact is no longer available as a live record")
        };
        let form = ContactForm::from_qso(qso);
        Ok(Self {
            expected,
            baseline: form.clone(),
            form,
            validation: Default::default(),
            errors: vec![],
            picker: None,
        })
    }

    /// Return the native identity retained by this saved editor.
    pub fn id(&self) -> anyhow::Result<QsoId> {
        match self.expected.key().id {
            RecordId::Qso(id) => Ok(id),
            _ => anyhow::bail!("Editor does not contain a contact"),
        }
    }
}

/// Individual choices remain part of the containing pending contact form.
struct EditorMembership {
    /// Checkbox state while the popup is open.
    picker: MembershipPicker,
    /// Pending form tags at popup opening.
    opening: BTreeSet<TagId>,
}

impl ContactsUi {
    /// Render the saved editor, retaining its expected record until an explicit reload.
    pub(super) fn show_editor(
        &mut self,
        context: &egui::Context,
        definitions: &DefinitionSet,
        requests: &mpsc::Sender<SettingsRequest>,
        catalog: &LogbookSnapshot,
    ) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let busy = self.operation.is_some();
        let mut save = None;
        let mut delete = false;
        let mut cancel = false;
        let mut reload = false;
        let result = crate::ui::dialog::show(
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
                    let quick = crate::ui::logging::entry::show(
                        ui,
                        &mut editor.form,
                        &editor.baseline,
                        &editor.errors,
                        false,
                        false,
                    );
                    enter = quick.save;
                    let mut changed = quick.edited;
                    changed |= editor.show_membership(ui, catalog);
                    ui.strong("Contact details");
                    let detail = crate::ui::logging::details::show(
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
                    let detail = crate::ui::logging::details::show(
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
            match ContactEditor::from_record(record) {
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
}

impl ContactEditor {
    /// Edit copied membership while the enclosing form retains its save basis.
    fn show_membership(&mut self, ui: &mut egui::Ui, catalog: &LogbookSnapshot) -> bool {
        let summary =
            MembershipPicker::new(catalog, std::slice::from_ref(&self.form.tags)).summary();
        let changed = self.form.tags != self.baseline.tags;
        let button = ui
            .horizontal(|ui| {
                ui.strong("Assigned logbooks");
                let response = ui.button("Edit…");
                ui.add(egui::Label::new(summary).truncate());
                if changed {
                    let colors = crate::ui::theme::palette(ui.ctx().theme());
                    ui.small(egui::RichText::new("Not applied").color(colors.edited_edge));
                }
                response
            })
            .inner;
        if button.clicked() {
            self.picker = Some(EditorMembership {
                picker: MembershipPicker::new(catalog, std::slice::from_ref(&self.form.tags)),
                opening: self.form.tags.clone(),
            });
        }
        let mut edited = false;
        egui::Popup::from_toggle_button_response(&button)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                if let Some(pending) = &mut self.picker {
                    edited = tag_picker::show_contents(ui, &mut pending.picker);
                    if edited {
                        let intent = pending.picker.intent();
                        self.form.tags = pending
                            .opening
                            .difference(&intent.remove)
                            .copied()
                            .collect();
                        self.form.tags.extend(intent.add);
                    }
                }
            });
        edited
    }
}
