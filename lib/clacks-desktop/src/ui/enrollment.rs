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
    /// Preserve user server edits when late startup context arrives.
    server_edited: bool,
    /// Installation label for browser approval.
    label: String,
    /// Whether this window's workspace is already enrolled.
    enrolled: bool,
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
            server: "https://clacks.w7hex.com".into(),
            server_edited: false,
            label: "Desktop".into(),
            enrolled: false,
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
                    server, enrolled, ..
                } => {
                    if (*enrolled || !self.open || !self.server_edited)
                        && let Some(server) = server
                    {
                        self.server = server.clone();
                    }
                    self.enrolled = *enrolled;
                    self.validation.invalidate();
                }
                EnrollmentStatus::Failed { enrolled, .. } => {
                    self.enrolled = *enrolled;
                }
                EnrollmentStatus::Authorizing { authorization_url } if self.open => {
                    context.open_url(egui::OpenUrl::new_tab(authorization_url))
                }
                EnrollmentStatus::Ready => {
                    self.enrolled = true;
                    self.open = false;
                    self.restore_focus = self.opener;
                    context.request_repaint();
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
                ui.label(if self.enrolled {"Sign in again to this workspace's existing account. Previously unsent work may need review if this client's access was revoked."}else{"Connect this app to your account. You will sign in and approve access in your browser."});
                let mut edited = fields::text(
                    ui,
                    egui::Id::unique("enrollment.server"),
                    "Server",
                    &mut self.server,
                    FieldPresentation {
                        edited: false,
                        read_only: busy || self.enrolled,
                        errors: &self.errors,
                        validation_label: None,
                        radio_data: false,
                    },
                )
                .changed();
                self.server_edited |= edited;
                edited |= fields::text(
                    ui,
                    egui::Id::unique("enrollment.label"),
                    "Client label",
                    &mut self.label,
                    FieldPresentation {
                        edited: false,
                        read_only: busy,
                        errors: &self.errors,
                        validation_label: None,
                        radio_data: false,
                    },
                )
                .changed();
                ui.small("Identifies this installation in your account's client list.");
                if edited {
                    self.validation.edited(now);
                }
                match &status {
                    EnrollmentStatus::Authorizing { authorization_url } => {
                        ui.label("Complete sign-in in your browser.");
                        if ui.button("Open browser").clicked() {
                            context.open_url(egui::OpenUrl::new_tab(authorization_url));
                        }
                    }
                    EnrollmentStatus::Saving => {
                        ui.label("Connecting…");
                    }
                    EnrollmentStatus::Ready => {}
                    EnrollmentStatus::Failed {
                        message, details, ..
                    } => {
                        super::feedback::problem(ui, message);
                        if let Some(details) = details {
                            ui.collapsing("Technical details", |ui| {
                                ui.label(details);
                            });
                        }
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
                            !busy && self.validation.can_submit(&self.errors),
                            egui::Button::new(if self.enrolled {
                                "Reauthorize"
                            } else {
                                "Connect"
                            }),
                        )
                        .clicked()
                    {
                        action = Some(EnrollmentCommand::Connect {
                            server: self.server.trim().into(),
                            label: self.label.trim().into(),
                        });
                    }
                    if ui.button("Cancel").clicked() {
                        dismiss = true;
                    }
                });
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
