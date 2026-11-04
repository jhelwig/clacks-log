//! One unfinished QSO shared by quick entry and adjacent details.

pub(super) mod details;
pub(in crate::ui) mod directory;
pub(in crate::ui) mod entry;
mod responses;
#[cfg(test)]
mod tests;
mod timestamps;

use super::{feedback, validation::ValidationState};
use crate::{
    logging::{defaults::NewEntry, form::ContactForm, time},
    operating::FieldError,
    session::settings::{
        SettingsAction, SettingsRequest, SettingsResponse, contacts::ContactAction,
    },
};
use clacks_client_store::{
    logbooks::selection::SelectedView, operating::OperatingState, settings::SettingsSnapshot,
};
use clacks_model::{contact::Qso, group::DefinitionSet};
use clacks_sync::record::{RecordState, RecordValue};
use eframe::egui;
use tokio::sync::{mpsc, oneshot};

/// Window-owned entry, exact submitted facts and its typed response.
pub(super) struct LoggingUi {
    /// Separate lookup preview and diagnosis.
    directory: directory::DirectoryView,
    /// One unfinished contact and its stable submission identity.
    entry: NewEntry,
    /// Copied input basis for changed-field presentation.
    baseline: ContactForm,
    /// Compiled account reference definitions used for explicit reference edits.
    definitions: DefinitionSet,
    /// Last actual group records; compile only when their stored values change.
    groups: Vec<clacks_sync::client::ClientRecord>,
    /// Whether feedback describes an unresolved operation rather than quiet success.
    problem: bool,
    /// Debounced validation readiness of the current text.
    validation: ValidationState,
    /// Actionable native validation feedback.
    errors: Vec<FieldError>,
    /// Acknowledgment receiver for the exact submitted contact.
    pending: Option<oneshot::Receiver<SettingsResponse>>,
    /// Frozen admitted facts, retained across uncertain outcomes.
    submitted: Option<Qso>,
    /// Latest committed operating defaults used for the next entry.
    active: OperatingState,
    /// Latest committed view supplying the next contact membership.
    view: SelectedView,
    /// Quiet success or persistent failure feedback.
    notice: Option<String>,
    /// Return keyboard focus to Callsign after a fresh-entry reset.
    focus_call: bool,
    /// Actual committed contact change, consumed by workbench list invalidation.
    pub changed: bool,
}

impl Default for LoggingUi {
    fn default() -> Self {
        let active = OperatingState::default();
        let view = SelectedView::AllContacts;
        let entry = NewEntry::new(&active, view, time::now());
        Self {
            directory: Default::default(),
            baseline: entry.form.clone(),
            entry,
            definitions: DefinitionSet::default(),
            groups: vec![],
            problem: false,
            validation: ValidationState::default(),
            errors: vec![],
            pending: None,
            submitted: None,
            active,
            view,
            notice: None,
            focus_call: false,
            changed: false,
        }
    }
}

impl LoggingUi {
    /// Accept the latest WS call without touching other unfinished facts.
    pub(super) fn incoming_call(&mut self, call: &str) {
        let call = call.trim().to_ascii_uppercase();
        if self.entry.form.dx.callsign == call {
            return;
        }
        self.entry.form.dx.callsign = call;
        if self.pending.is_none() {
            self.submitted = None;
        }
        self.validation.invalidate();
        self.directory
            .observe(&self.entry.form.dx.callsign, tokio::time::Instant::now());
    }

    /// Use the same fresh draft initialization as a successful manual save.
    pub(super) fn clear(&mut self) {
        if self.pending.is_some() {
            return;
        }
        self.entry.reset(&self.active, self.view, time::now());
        self.baseline = self.entry.form.clone();
        self.directory = Default::default();
        self.submitted = None;
        self.validation.invalidate();
        self.errors.clear();
        self.problem = false;
        self.notice = None;
        self.focus_call = true;
    }

    /// Advance shared lookup interest independently of which entry pane is visible.
    pub(super) fn advance_lookup(
        &mut self,
        context: &egui::Context,
        lookup: Option<&crate::directory::lookup::LookupClient>,
    ) {
        self.poll();
        self.directory
            .observe(&self.entry.form.dx.callsign, tokio::time::Instant::now());
        self.directory
            .advance(context, lookup, tokio::time::Instant::now());
    }

    /// Borrow the unfinished contacted call for Worked Before without exposing the draft.
    pub fn contact_call(&self) -> &str {
        &self.entry.form.dx.callsign
    }
    /// Follow current defaults when editable, preserving the submitted value while busy.
    pub fn refresh(&mut self, snapshot: &SettingsSnapshot) {
        let defaults_changed =
            self.active != snapshot.operating || self.view != snapshot.logbooks.selected_view;
        self.active = snapshot.operating.clone();
        self.view = snapshot.logbooks.selected_view;
        if self.groups != snapshot.groups {
            self.groups = snapshot.groups.clone();
            let definitions = snapshot
                .groups
                .iter()
                .filter_map(|v| match v.working_state() {
                    Some(RecordState::Live(RecordValue::Group(value))) => {
                        Some(value.as_ref().clone())
                    }
                    _ => None,
                })
                .collect();
            match DefinitionSet::new(definitions) {
                Ok(value) => self.definitions = value,
                Err(error) => {
                    self.notice = Some(format!("Reference definitions need attention: {error}"))
                }
            }
        }
        if self.pending.is_none() && self.submitted.is_none() {
            self.entry.refresh_defaults(&self.active, self.view);
            if defaults_changed {
                let defaults = crate::operating::editor::OperatingEditor::new(&self.active);
                self.baseline.own = defaults.station;
                self.baseline.equipment = defaults.equipment;
                self.baseline.operator = defaults.operator_callsign;
                self.baseline.operator_name = defaults.operator_name;
                self.baseline.power = defaults.power_watts;
                self.validation.invalidate();
            }
        }
    }

    /// Share the account definitions already compiled for contact entry.
    pub fn definitions(&self) -> &DefinitionSet {
        &self.definitions
    }

    /// Render quick entry; adjacent details are drawn separately into the same draft.
    pub fn show(&mut self, ui: &mut egui::Ui, requests: &mpsc::Sender<SettingsRequest>) {
        self.poll();
        let clock = time::now();
        if self.pending.is_none()
            && self.submitted.is_none()
            && let Err(error) = self
                .entry
                .form
                .start
                .tick(clock)
                .and_then(|_| self.entry.form.end.tick(clock))
        {
            self.notice = Some(error.to_string());
            self.problem = true;
        }
        if self.pending.is_none() && (self.entry.form.start.live || self.entry.form.end.live) {
            let nanos = 1_000_000_000 - u64::from(::time::OffsetDateTime::now_utc().nanosecond());
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_nanos(nanos));
        }
        let now = ui.input(|i| i.time);
        if self.validation.due(now) {
            self.errors = self
                .entry
                .form
                .build(self.entry.id, &self.definitions)
                .err()
                .unwrap_or_default();
            self.validation.validated();
        }
        self.validation.request_repaint(ui.ctx(), now);
        let enabled = self.pending.is_none() && self.validation.can_submit(&self.errors);
        let item_spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing.y = 0.0;
        let header = super::theme::form_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("QSO entry");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save = ui
                        .add_enabled(enabled, egui::Button::new("Save QSO"))
                        .clicked();
                    if ui
                        .add_enabled(self.pending.is_none(), egui::Button::new("Clear"))
                        .clicked()
                    {
                        self.clear();
                    }
                    save
                })
                .inner
            })
            .inner
        });
        ui.spacing_mut().item_spacing = item_spacing;
        ui.painter().hline(
            header.response.rect.x_range(),
            header.response.rect.bottom(),
            egui::Stroke::new(1.0, super::theme::palette(ui.ctx().theme()).border),
        );
        let clicked = header.inner;
        egui::ScrollArea::vertical()
            .id_salt("entry.quick.scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                super::theme::form_frame().show(ui, |ui| {
                    let before = self.entry.form.clone();
                    let fallback = self.directory.fallback(&self.entry.form, &self.definitions);
                    let action = ui
                        .add_enabled_ui(self.pending.is_none(), |ui| {
                            entry::show(
                                ui,
                                &mut self.entry.form,
                                &self.baseline,
                                &self.errors,
                                self.focus_call,
                                true,
                                &fallback,
                            )
                        })
                        .inner;
                    self.focus_call = false;
                    self.directory
                        .observe(&self.entry.form.dx.callsign, tokio::time::Instant::now());
                    self.directory.show(ui);
                    if action.edited {
                        self.entry.capture_corrections(&before);
                        self.submitted = None;
                        self.validation.edited(now);
                    }
                    if !self.errors.is_empty() {
                        for error in &self.errors {
                            ui.small(
                                egui::RichText::new(format!("{}: {}", error.field, error.message))
                                    .color(ui.visuals().error_fg_color),
                            );
                        }
                    }
                    if self.pending.is_some() {
                        ui.label("Saving…");
                    }
                    if (clicked || action.save) && self.pending.is_none() {
                        self.save(requests);
                    }
                    if let Some(notice) = &self.notice {
                        feedback::problem(ui, notice);
                    }
                });
            });
    }

    /// Present either additional contact facts or contact-specific own settings.
    pub fn details(
        &mut self,
        ui: &mut egui::Ui,
        requests: &mpsc::Sender<SettingsRequest>,
        own: bool,
    ) {
        let before = self.entry.form.clone();
        let fallback = self.directory.fallback(&self.entry.form, &self.definitions);
        let changed = ui
            .add_enabled_ui(self.pending.is_none(), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("entry.details")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        super::theme::form_frame()
                            .show(ui, |ui| {
                                details::show(
                                    ui,
                                    &mut self.entry.form,
                                    &self.baseline,
                                    &self.definitions,
                                    &self.errors,
                                    crate::ui::logging::details::DetailPresentation {
                                        own,
                                        owner: "entry.details",
                                        fallback: &fallback,
                                    },
                                )
                            })
                            .inner
                    })
                    .inner
            })
            .inner;
        if changed.edited {
            self.entry.capture_corrections(&before);
            self.submitted = None;
            self.validation.edited(ui.input(|i| i.time));
        }
        self.directory
            .observe(&self.entry.form.dx.callsign, tokio::time::Instant::now());
        if changed.save && self.pending.is_none() {
            self.save(requests);
        }
    }

    /// Submit exactly the validated contact and retain its ID for uncertain retries.
    fn save(&mut self, requests: &mpsc::Sender<SettingsRequest>) {
        let next = match self.entry.submit(time::now(), &self.definitions) {
            Ok(value) => value,
            Err(errors) => {
                self.errors = errors;
                return;
            }
        };
        let (reply, receiver) = oneshot::channel();
        let request = SettingsRequest {
            action: SettingsAction::Contacts(ContactAction::Save {
                directory: self.directory.submission(),
                expected: None,
                next: Box::new(next.clone()),
            }),
            reply,
        };
        match requests.try_send(request) {
            Ok(()) => {
                self.submitted = Some(next);
                self.pending = Some(receiver);
                self.notice = None;
            }
            Err(error) => {
                self.notice = Some(format!(
                    "Unable to save: {error}. Retry when the workspace is ready."
                ))
            }
        }
    }
}
