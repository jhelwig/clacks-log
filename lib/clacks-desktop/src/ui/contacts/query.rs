//! Debounced expression editing, result provenance and coalesced local persistence.

mod suggestions;
#[cfg(test)]
mod tests;

use crate::{
    session::settings::{
        SettingsAction, SettingsRequest, SettingsResponse,
        contacts::{ContactAction, ContactResponse},
    },
    ui::{
        fields::{self, FieldPresentation},
        theme,
    },
};
use clacks_client_store::{read::query_state::QueryState, settings::SettingsSnapshot};
use clacks_model::{
    group::DefinitionSet,
    query::{self, Query, diagnostic::QueryDiagnostic},
    recognition::RecognitionCatalog,
};
use eframe::egui;
use tokio::sync::{mpsc, oneshot};

/// Quiet interval before validating and requesting a changed expression.
const DEBOUNCE: f64 = 0.250;

/// One acknowledged local preference write and its exact submitted pair.
struct StateWrite {
    /// State whose durability is being acknowledged.
    state: QueryState,
    /// The runtime's own response channel.
    receiver: oneshot::Receiver<SettingsResponse>,
}

/// One workspace's shared query editor; selection and rows belong to ContactsUi.
#[derive(Default)]
pub(in crate::ui) struct HistoryQueryUi {
    /// Unfinished edit and expression actually supplying the displayed results.
    pub state: QueryState,
    /// Latest validated desired expression, absent until restoration succeeds.
    pub desired: Option<Query>,
    /// Text associated with the desired expression, including in-flight results.
    pub desired_text: String,
    /// Current debounced syntax diagnostic.
    diagnostic: Option<QueryDiagnostic>,
    /// Next parse and persistence opportunity.
    deadline: Option<f64>,
    /// Restoration is complete, including an actionable failure.
    loaded: bool,
    /// Exact asynchronous restoration request.
    loading: Option<oneshot::Receiver<SettingsResponse>>,
    /// Last pair known durable, not merely submitted.
    saved: Option<QueryState>,
    /// At most one preference write in flight.
    writing: Option<StateWrite>,
    /// Failure retained until explicit retry or a new edit.
    pub error: Option<String>,
    /// Suggestions were explicitly dismissed for this edit/caret.
    dismissed: bool,
    /// Current keyboard choice in the bounded native popup.
    choice: usize,
}

impl HistoryQueryUi {
    /// Restore once and advance debounced parsing/persistence without blocking rendering.
    pub fn advance(
        &mut self,
        context: &egui::Context,
        requests: &mpsc::Sender<SettingsRequest>,
        flush: bool,
    ) {
        let now = context.input(|i| i.time);
        if !self.loaded && self.loading.is_none() && self.error.is_none() {
            self.loading = send(requests, ContactAction::LoadQueryState).ok();
        }
        if let Some(receiver) = &mut self.loading {
            match receiver.try_recv() {
                Ok(SettingsResponse::Contacts(ContactResponse::QueryState(state))) => {
                    self.loading = None;
                    self.loaded = true;
                    self.state = state.clone();
                    self.saved = Some(state);
                    match RecognitionCatalog::adif_318()
                        .map_err(|e| e.to_string())
                        .and_then(|c| {
                            query::parse(&self.state.applied, c).map_err(|e| e.to_string())
                        }) {
                        Ok(query) => {
                            self.desired = Some(query);
                            self.desired_text = self.state.applied.clone();
                        }
                        Err(error) => {
                            self.error =
                                Some(format!("Cannot restore applied history query: {error}"))
                        }
                    }
                    self.deadline = Some(now + DEBOUNCE);
                }
                Ok(response) => {
                    self.loading = None;
                    self.error = Some(response_error(response));
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.loading = None;
                    self.error =
                        Some("History query restoration ended without acknowledgment".into());
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
        }
        if let Some(write) = &mut self.writing {
            match write.receiver.try_recv() {
                Ok(SettingsResponse::Contacts(ContactResponse::QueryStateSaved(state)))
                    if state == write.state =>
                {
                    self.saved = Some(state);
                    self.writing = None;
                }
                Ok(response) => {
                    self.writing = None;
                    self.error = Some(response_error(response));
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.writing = None;
                    self.error = Some("History query save ended without acknowledgment".into());
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
        }
        if self.loaded && self.deadline.is_some_and(|at| now >= at) {
            self.deadline = None;
            match RecognitionCatalog::adif_318() {
                Ok(catalog) => match query::parse(&self.state.edited, catalog) {
                    Ok(query) => {
                        self.desired = Some(query);
                        self.desired_text = self.state.edited.clone();
                        self.diagnostic = None;
                    }
                    Err(error) => self.diagnostic = Some(error),
                },
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if self.loaded
            && self.error.is_none()
            && self.writing.is_none()
            && self.saved.as_ref() != Some(&self.state)
            && (flush || self.deadline.is_none())
        {
            match send(
                requests,
                ContactAction::SaveQueryState {
                    state: self.state.clone(),
                },
            ) {
                Ok(receiver) => {
                    self.writing = Some(StateWrite {
                        state: self.state.clone(),
                        receiver,
                    })
                }
                Err(error) => self.error = Some(error),
            }
        }
        if let Some(at) = self.deadline {
            context.request_repaint_after(std::time::Duration::from_secs_f64((at - now).max(0.0)));
        }
        if self.loading.is_some() || self.writing.is_some() {
            context.request_repaint_after(std::time::Duration::from_millis(25));
        }
    }

    /// Associate accepted result rows with their original expression text.
    pub fn applied(&mut self, text: String) {
        self.state.applied = text;
    }

    /// Flush the latest text pair before the runtime closes request admission.
    pub fn flush(
        &mut self,
        context: &egui::Context,
        requests: &mpsc::Sender<SettingsRequest>,
    ) -> bool {
        if !self.loaded && self.loading.is_none() && self.error.is_none() {
            return true;
        }
        self.advance(context, requests, true);
        // A never-opened editor has nothing to overwrite or save.
        (!self.loaded && self.loading.is_none() && self.error.is_none())
            || (self.loaded
                && self.error.is_none()
                && self.writing.is_none()
                && self.saved.as_ref() == Some(&self.state))
    }

    /// Retry a failed preference operation without changing either expression.
    pub fn retry(&mut self) {
        self.error = None;
    }

    /// Show the editor and completion popup within the existing history section.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &SettingsSnapshot,
        definitions: &DefinitionSet,
    ) {
        let now = ui.input(|i| i.time);
        let colors = theme::palette(ui.ctx().theme());
        let pending = self.state.edited != self.state.applied;
        let row_height = 2.0 * ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
        let action_width = egui::WidgetText::from("Clear query")
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            )
            .size()
            .x
            + 2.0 * ui.spacing().button_padding.x;
        let mut response = None;
        let mut clear = false;
        ui.allocate_ui(egui::vec2(ui.available_width(), row_height), |ui| {
            egui_extras::StripBuilder::new(ui)
                .size(egui_extras::Size::remainder())
                .size(egui_extras::Size::exact(action_width))
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        ui.spacing_mut().text_edit_width = ui.available_width();
                        response = Some(fields::text(
                            ui,
                            egui::Id::unique("history.query"),
                            "Filter contacts",
                            &mut self.state.edited,
                            FieldPresentation {
                                edited: pending,
                                read_only: !self.loaded,
                                errors: &[],
                                validation_label: None,
                                radio_data: true,
                            },
                        ));
                    });
                    strip.cell(|ui| {
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                            clear = ui
                                .add_enabled(
                                    self.loaded && !self.state.edited.is_empty(),
                                    egui::Button::new("Clear query"),
                                )
                                .clicked();
                        });
                    });
                });
        });
        let Some(response) = response else {
            return;
        };
        if clear {
            self.state.edited.clear();
        }
        if response.changed() || clear {
            self.edited(now);
        }
        let status = if let Some(error) = &self.error {
            error.clone()
        } else if let Some(error) = &self.diagnostic {
            let character = self
                .state
                .edited
                .get(..error.span.start)
                .map_or(1, |text| text.chars().count() + 1);
            format!(
                "{} (at character {}). Showing: {}",
                error.message,
                character,
                displayed(&self.state.applied)
            )
        } else if pending {
            format!("Showing: {}", displayed(&self.state.applied))
        } else {
            "Use field:value, AND, OR, NOT and parentheses. Unqualified text matches callsigns."
                .into()
        };
        ui.add(
            egui::Label::new(egui::RichText::new(status).small().color(
                if self.error.is_some() || self.diagnostic.is_some() {
                    colors.error
                } else {
                    colors.muted
                },
            ))
            .truncate(),
        );
        if self.error.is_some() && ui.small_button("Retry history query").clicked() {
            self.error = None;
            self.deadline = Some(now);
        }
        if response.has_focus() || response.lost_focus() {
            self.suggestions(ui, &response, snapshot, definitions, now);
        }
    }

    /// Restart only this expression's quiet interval after an actual edit.
    fn edited(&mut self, now: f64) {
        self.deadline = Some(now + DEBOUNCE);
        self.diagnostic = None;
        self.dismissed = false;
        self.choice = 0;
        self.error = None;
    }
}

/// Keep an empty expression's display meaning explicit without changing its text.
fn displayed(text: &str) -> &str {
    if text.trim().is_empty() {
        "all contacts in this view"
    } else {
        text
    }
}

/// Submit a real contact preference operation through the existing runtime channel.
fn send(
    requests: &mpsc::Sender<SettingsRequest>,
    action: ContactAction,
) -> Result<oneshot::Receiver<SettingsResponse>, String> {
    let (reply, receiver) = oneshot::channel();
    requests
        .try_send(SettingsRequest {
            action: SettingsAction::Contacts(action),
            reply,
        })
        .map_err(|e| format!("Cannot request history preferences: {e}"))?;
    Ok(receiver)
}

/// Surface the runtime's actionable failure without discarding expression text.
fn response_error(response: SettingsResponse) -> String {
    match response {
        SettingsResponse::Failed(message) => message,
        SettingsResponse::Closing => {
            "Workspace is closing before history preferences were saved".into()
        }
        _ => "Unexpected history preference acknowledgment; retry before closing".into(),
    }
}
