//! Inline operating power commits through the existing guarded settings transaction.

use super::*;
use crate::{
    operating::power,
    ui::{theme, validation::ValidationState},
};
use clacks_client_store::operating::{OperatingSaveOutcome, OperatingState};

/// A submitted power edit and its originating reply, independent of later typing.
struct PowerRequest {
    /// Text submitted for this particular request.
    submitted: String,
    /// Exactly one guarded settings result.
    reply: oneshot::Receiver<SettingsResponse>,
}

/// Result needed by the summary's owning committed-state view.
#[derive(Default)]
pub(super) struct QuickOutcome {
    /// Accepted basis to show even if the following view read fails.
    pub acknowledged: Option<OperatingState>,
    /// Failed post-operation read, distinct from the write outcome.
    pub refresh_error: Option<String>,
}

/// A power field independent of any full operating form.
#[derive(Default)]
pub(super) struct QuickOperatingUi {
    /// Incomplete text retained until explicit commit or cancellation.
    pub(super) power_text: String,
    /// Whether the text is an operator edit rather than a reflected committed value.
    dirty: bool,
    /// Live validation freshness using the framework clock.
    validation: ValidationState,
    /// Current native parse failure.
    error: Option<String>,
    /// Request failure independent of debounced input validation.
    notice: Option<String>,
    /// Admitted edit; dropping focus never discards its response.
    power_request: Option<PowerRequest>,
}

impl QuickOperatingUi {
    /// Settle admitted power work without replacing text entered after submission.
    pub fn poll(&mut self) -> QuickOutcome {
        let Some(request) = &mut self.power_request else {
            return QuickOutcome::default();
        };
        let response = match request.reply.try_recv() {
            Ok(response) => response,
            Err(oneshot::error::TryRecvError::Empty) => return QuickOutcome::default(),
            Err(oneshot::error::TryRecvError::Closed) => {
                self.power_request = None;
                self.notice =
                    Some("No acknowledgment received. Refresh settings before retrying.".into());
                return QuickOutcome::default();
            }
        };
        let submitted = request.submitted.clone();
        self.power_request = None;
        let mut outcome = QuickOutcome::default();
        let response = if let SettingsResponse::RefreshFailed {
            outcome: response,
            message,
        } = response
        {
            outcome.refresh_error = Some(message);
            *response
        } else {
            response
        };
        match response {
            SettingsResponse::Apply(
                OperatingSaveOutcome::Saved(state) | OperatingSaveOutcome::Unchanged(state),
            ) => {
                if self.power_text == submitted {
                    self.power_text = power::format_watts(
                        state
                            .current
                            .as_ref()
                            .and_then(|value| value.transmit_power),
                    );
                    self.dirty = false;
                    self.error = None;
                }
                outcome.acknowledged = Some(state);
            }
            SettingsResponse::Apply(OperatingSaveOutcome::Stale(_)) => {
                self.notice = Some("Operating settings changed. Your power edit was not applied. Review the active values, then retry.".into());
            }
            SettingsResponse::Failed(message) => self.notice = Some(message),
            SettingsResponse::Closing => {
                self.notice = Some("The application is closing; power was not applied.".into())
            }
            _ => self.notice = Some("Unexpected settings reply. Refresh before retrying.".into()),
        }
        outcome
    }

    /// Report whether an admitted power edit still awaits acknowledgment.
    pub fn is_busy(&self) -> bool {
        self.power_request.is_some()
    }

    /// Render decimal watts inline, committing valid input on Enter or focus departure.
    pub fn power(
        &mut self,
        ui: &mut egui::Ui,
        active: &OperatingState,
        sender: &mpsc::Sender<SettingsRequest>,
    ) {
        let committed = active
            .current
            .as_ref()
            .and_then(|value| value.transmit_power);
        if !self.dirty && self.power_request.is_none() {
            self.power_text = power::format_watts(committed);
        }
        let now = ui.input(|input| input.time);
        if self.dirty && self.validation.due(now) {
            self.error = power::parse_watts(&self.power_text).err();
            self.validation.validated();
        }
        self.validation.request_repaint(ui.ctx(), now);
        let colors = theme::palette(ui.ctx().theme());
        let mut response = ui
            .add_enabled_ui(active.current.is_some(), |ui| {
                ui.horizontal(|ui| {
                    let label = ui.label("Power");
                    let mut edit = egui::TextEdit::singleline(&mut self.power_text)
                        .id(egui::Id::unique("summary.power"))
                        .font(egui::TextStyle::Monospace)
                        .desired_width(84.0)
                        .hint_text("—");
                    if self.dirty {
                        edit = edit.background_color(colors.edited_fill);
                    }
                    let response = ui.add(edit).labelled_by(label.id);
                    ui.label("W");
                    response
                })
                .inner
            })
            .inner;
        if active.current.is_none() {
            response = response.on_disabled_hover_text(
                "Set up a station in Operating settings before applying power.",
            );
        }
        if response.changed() {
            self.dirty = self.power_request.is_some()
                || power::parse_watts(&self.power_text)
                    .map(|value| value != committed)
                    .unwrap_or(true);
            self.validation.edited(now);
        }
        if self.dirty {
            ui.small(egui::RichText::new("Not applied").color(colors.edited_edge));
            ui.painter().vline(
                response.rect.left() + 2.0,
                response.rect.y_range(),
                egui::Stroke::new(2.0, colors.edited_edge),
            );
        }
        let editing = response.has_focus() || response.lost_focus();
        let cancelled = editing
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if cancelled {
            self.power_text = power::format_watts(committed);
            self.dirty = false;
            self.error = None;
            response.surrender_focus();
        } else if self.dirty
            && self.power_request.is_none()
            && (response.lost_focus()
                || (response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))))
        {
            self.commit(active, sender);
        }
        if let Some(error) = &self.error {
            ui.colored_label(colors.error, error);
        }
        if let Some(notice) = &self.notice {
            crate::ui::feedback::problem(ui, notice);
        }
        if self.is_busy() {
            ui.small("Applying power…");
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    /// Apply only power against the latest complete operating basis.
    fn commit(&mut self, active: &OperatingState, sender: &mpsc::Sender<SettingsRequest>) {
        let value = match power::parse_watts(&self.power_text) {
            Ok(value) => value,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let mut next = active.clone();
        self.validation.validated();
        let Some(current) = &mut next.current else {
            return;
        };
        current.transmit_power = value;
        let (reply, response) = oneshot::channel();
        match sender.try_send(SettingsRequest {
            action: SettingsAction::Apply {
                expected: active.clone(),
                next,
            },
            reply,
        }) {
            Ok(()) => {
                self.power_request = Some(PowerRequest {
                    submitted: self.power_text.clone(),
                    reply: response,
                });
                self.error = None;
                self.notice = None;
            }
            Err(error) => self.notice = Some(format!("Could not apply power: {error}")),
        }
    }
}
