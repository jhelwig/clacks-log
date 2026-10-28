//! One unfinished QSO shared by quick entry and adjacent details.

pub(super) mod details;
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

/// The two delivered adjacent views; Worked Before is a later real capability.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    Contact,
    OurSettings,
}

/// Window-owned entry, exact submitted facts and its typed response.
pub(super) struct LoggingUi {
    /// One unfinished contact and its stable submission identity.
    entry: NewEntry,
    /// Copied input basis for changed-field presentation.
    baseline: ContactForm,
    /// Currently visible additional-detail view.
    tab: DetailTab,
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
    /// Return keyboard focus to Callsign after a successful new save.
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
            baseline: entry.form.clone(),
            entry,
            tab: DetailTab::Contact,
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
        ui.strong("QSO entry");
        ui.separator();
        let before = self.entry.form.clone();
        let action = ui
            .add_enabled_ui(self.pending.is_none(), |ui| {
                entry::show(
                    ui,
                    &mut self.entry.form,
                    &self.baseline,
                    &self.errors,
                    self.focus_call,
                    true,
                )
            })
            .inner;
        self.focus_call = false;
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
        let enabled = self.pending.is_none() && self.validation.can_submit(&self.errors);
        let clicked = ui
            .add_enabled(enabled, egui::Button::new("Save QSO"))
            .clicked();
        if self.pending.is_some() {
            ui.label("Saving…");
        }
        if (clicked || action.save) && self.pending.is_none() {
            self.save(requests);
        }
        if let Some(notice) = &self.notice {
            feedback::problem(ui, notice);
        }
    }

    /// Present either additional contact facts or contact-specific own settings.
    pub fn details(&mut self, ui: &mut egui::Ui, requests: &mpsc::Sender<SettingsRequest>) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, DetailTab::Contact, "Contact details");
            ui.selectable_value(&mut self.tab, DetailTab::OurSettings, "Our settings");
        });
        ui.separator();
        let before = self.entry.form.clone();
        let changed = ui
            .add_enabled_ui(self.pending.is_none(), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("entry.details")
                    .show(ui, |ui| {
                        details::show(
                            ui,
                            &mut self.entry.form,
                            &self.baseline,
                            &self.definitions,
                            &self.errors,
                            self.tab == DetailTab::OurSettings,
                            "entry.details",
                        )
                    })
                    .inner
            })
            .inner;
        if changed.edited {
            self.entry.capture_corrections(&before);
            self.submitted = None;
            self.validation.edited(ui.input(|i| i.time));
        }
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
