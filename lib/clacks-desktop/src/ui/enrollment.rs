//! Connection dialog using the shared modal, fields and debounced validation.

#[cfg(test)]
mod tests;

use super::{
    fields::{self, FieldPresentation},
    validation::ValidationState,
};
use crate::{
    enrollment::{EnrollmentCommand, EnrollmentStatus},
    operating::{FieldError, issue},
    session::SessionCommand,
};
use eframe::egui;
use tokio::sync::{mpsc, watch};

/// Transient connection input and the latest nonsecret runtime context.
pub(super) struct EnrollmentUi {
    /// Whether the connection modal is visible.
    open: bool,
    /// Deployment origin typed or loaded from an existing binding.
    server: String,
    /// Installation label for browser approval.
    label: String,
    /// Whether this window's workspace is already enrolled.
    enrolled: bool,
    /// A durable attempt is available to resume.
    resume_available: bool,
    /// A pre-binding attempt can be explicitly abandoned.
    discard_available: bool,
    /// A request is queued before its first runtime status arrives.
    submitted: bool,
    /// Shared validation debounce, independent of operation progress.
    validation: ValidationState,
    /// Current field-specific validation results.
    errors: Vec<FieldError>,
    /// Action admission error that leaves the form available for retry.
    notice: Option<String>,
    /// Native control whose focus should return after dismissal.
    opener: Option<egui::Id>,
    /// Focus request deferred until the modal has left the active frame.
    restore_focus: Option<egui::Id>,
}

impl Default for EnrollmentUi {
    fn default() -> Self {
        Self {
            open: false,
            server: String::new(),
            label: "Desktop".into(),
            enrolled: false,
            resume_available: false,
            discard_available: false,
            submitted: false,
            validation: Default::default(),
            errors: Vec::new(),
            notice: None,
            opener: None,
            restore_focus: None,
        }
    }
}

impl EnrollmentUi {
    /// Current modal visibility participates in menu availability.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the connection workflow without resetting its inputs.
    pub fn open(&mut self, opener: Option<egui::Id>) {
        if !self.open {
            self.open = true;
            self.opener = opener;
            self.validation.invalidate();
        }
    }

    /// Present current progress, explicit retries and validated native authorization requests.
    pub fn show(
        &mut self,
        context: &egui::Context,
        commands: &mpsc::Sender<SessionCommand>,
        view: &mut watch::Receiver<EnrollmentStatus>,
    ) {
        if let Some(opener) = self.restore_focus.take() {
            context.memory_mut(|memory| memory.request_focus(opener));
        }
        let changed = view.has_changed().unwrap_or(false);
        let status = view.borrow_and_update().clone();
        if changed {
            self.submitted = false;
            match &status {
                EnrollmentStatus::Idle {
                    server,
                    enrolled,
                    resume_available,
                    discard_available,
                    ..
                } => {
                    if !self.open || self.server.is_empty() {
                        self.server = server.clone().unwrap_or_default();
                    }
                    self.enrolled = *enrolled;
                    self.resume_available = *resume_available;
                    self.discard_available = *discard_available;
                    self.validation.invalidate();
                }
                EnrollmentStatus::Failed {
                    resume_available,
                    discard_available,
                    ..
                } => {
                    self.resume_available = *resume_available;
                    self.discard_available = *discard_available;
                }
                EnrollmentStatus::Authorizing { authorization_url } if self.open => {
                    context.open_url(egui::OpenUrl::new_tab(authorization_url))
                }
                EnrollmentStatus::Ready { .. } => {
                    self.resume_available = false;
                    self.discard_available = false;
                }
                _ => {}
            }
        }
        if !self.open {
            return;
        }
        let busy = self.submitted
            || matches!(
                status,
                EnrollmentStatus::Authorizing { .. } | EnrollmentStatus::Saving
            );
        let now = context.input(|input| input.time);

        if self.validation.due(now) {
            self.errors.clear();
            if let Err(error) = crate::enrollment::flow::origin(&self.server) {
                self.errors.push(issue("Server", error));
            }
            if self.label.trim().is_empty() || self.label.chars().count() > 128 {
                self.errors
                    .push(issue("Client label", "Enter 1–128 characters."));
            }
            self.validation.validated();
        }
        self.validation.request_repaint(context, now);
        let mut action = None;
        let mut dismiss = false;
        let response = super::dialog::show(
            context,
            egui::Id::unique("enrollment.dialog"),
            if self.enrolled {
                "Client authorization"
            } else {
                "Connect to server"
            },
            |ui| {
                ui.style_mut().spacing.text_edit_width = ui.available_width();
                ui.label(if self.enrolled {"Reauthorize this workspace with its existing account. A closed or unknown client receives a new writer identity; old pending work will require review."}else{"Connect this app to your account. Empty logs connect here. Existing local data is preserved; populated logs require export and import."});
                let mut edited = fields::text(
                    ui,
                    egui::Id::unique("enrollment.server"),
                    "Server",
                    &mut self.server,
                    FieldPresentation {
                        edited: false,
                        read_only: busy || self.enrolled || self.resume_available,
                        errors: &self.errors,
                        validation_label: None,
                        radio_data: false,
                    },
                )
                .changed();
                edited |= fields::text(
                    ui,
                    egui::Id::unique("enrollment.label"),
                    "Client label",
                    &mut self.label,
                    FieldPresentation {
                        edited: false,
                        read_only: busy || (self.resume_available && !self.discard_available),
                        errors: &self.errors,
                        validation_label: None,
                        radio_data: false,
                    },
                )
                .changed();
                if edited {
                    self.validation.edited(now);
                }
                match &status {
                    EnrollmentStatus::Authorizing { authorization_url } => {
                        ui.label("Complete authorization in your browser.");
                        if ui.button("Open browser").clicked() {
                            context.open_url(egui::OpenUrl::new_tab(authorization_url));
                        }
                    }
                    EnrollmentStatus::Saving => {
                        ui.label("Saving client authorization…");
                    }
                    EnrollmentStatus::Ready { destination } => {
                        ui.label("Enrolled; awaiting synchronization.");
                        ui.label(destination.display().to_string());
                    }
                    EnrollmentStatus::Failed { message, .. } => {
                        super::feedback::problem(ui, message);
                    }
                    EnrollmentStatus::Idle { .. } => {}
                }
                if let Some(notice) = &self.notice {
                    super::feedback::problem(ui, notice);
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !busy
                                && !self.resume_available
                                && self.validation.can_submit(&self.errors),
                            egui::Button::new(if self.enrolled {
                                "Reauthorize"
                            } else {
                                "Connect"
                            }),
                        )
                        .clicked()
                    {
                        action = Some(EnrollmentCommand::Begin {
                            server: self.server.trim().into(),
                            label: self.label.trim().into(),
                        });
                    }
                    if self.resume_available
                        && ui
                            .add_enabled(!busy, egui::Button::new("Resume saved attempt"))
                            .clicked()
                    {
                        action = Some(EnrollmentCommand::Resume {
                            label: self.label.trim().into(),
                        });
                    }
                    if self.resume_available
                        && !self.discard_available
                        && ui
                            .add_enabled(!busy, egui::Button::new("Authorize again in browser"))
                            .clicked()
                    {
                        action = Some(EnrollmentCommand::AuthorizeAgain {
                            label: self.label.trim().into(),
                        });
                    }
                    if self.discard_available
                        && ui
                            .add_enabled(!busy, egui::Button::new("Discard saved attempt"))
                            .clicked()
                    {
                        action = Some(EnrollmentCommand::Discard);
                    }
                    if ui
                        .button(if matches!(status, EnrollmentStatus::Ready { .. }) {
                            "Close"
                        } else {
                            "Cancel"
                        })
                        .clicked()
                    {
                        dismiss = true;
                    }
                });
                if self.discard_available {
                    ui.small("Discard removes the unfinished attempt's local key; it does not revoke a completed server authorization.");
                }
            },
        );
        if let Some(action) = action {
            match commands.try_send(SessionCommand::Enrollment(action)) {
                Ok(()) => {
                    self.submitted = true;
                    self.notice = None;
                }
                Err(_) => {
                    self.notice =
                        Some("The workspace is busy. Try the connection action again.".into())
                }
            }
        }
        if dismiss || response.cancel_requested {
            if busy
                && commands
                    .try_send(SessionCommand::Enrollment(EnrollmentCommand::Cancel))
                    .is_err()
            {
                self.notice = Some("The workspace is busy. Retry Cancel.".into());
                return;
            }
            self.open = false;
            self.restore_focus = self.opener;
            context.request_repaint();
        }
    }
}
