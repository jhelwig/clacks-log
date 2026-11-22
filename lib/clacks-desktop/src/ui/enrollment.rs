//! Connection section using the shared fields and debounced validation.

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
    /// Whether Settings owns the connection inputs.
    open: bool,
    /// Deployment origin typed or loaded from an existing binding.
    server: String,
    /// Connection origin restored when the draft is dismissed.
    saved_server: String,
    /// Preserve user server edits when late startup context arrives.
    server_edited: bool,
    /// Installation label for browser approval.
    label: String,
    /// Last installation label submitted successfully.
    saved_label: String,
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
}

impl Default for EnrollmentUi {
    fn default() -> Self {
        Self {
            open: false,
            server: "https://clacks.w7hex.com".into(),
            saved_server: "https://clacks.w7hex.com".into(),
            server_edited: false,
            label: "Desktop".into(),
            saved_label: "Desktop".into(),
            enrolled: false,
            submitted: false,
            validation: Default::default(),
            errors: Vec::new(),
            notice: None,
        }
    }
}

impl EnrollmentUi {
    /// Begin editing the Connection section as part of Settings.
    pub fn open(&mut self) {
        self.open = true;
        self.validation.invalidate();
        self.notice = None;
    }

    /// Publish connection progress even when the Connection section is hidden.
    pub fn poll(&mut self, context: &egui::Context, view: &mut watch::Receiver<EnrollmentStatus>) {
        let changed = view.has_changed().unwrap_or(false);
        let status = view.borrow_and_update().clone();
        if changed {
            self.submitted = false;
            match &status {
                EnrollmentStatus::Idle {
                    server, enrolled, ..
                } => {
                    if let Some(server) = server {
                        self.saved_server = server.clone();
                        if *enrolled || !self.open || !self.server_edited {
                            self.server = server.clone();
                        }
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
                    self.saved_server.clone_from(&self.server);
                    self.saved_label.clone_from(&self.label);
                    context.request_repaint();
                }
                _ => {}
            }
        }
    }

    /// Whether enrollment owns the workspace or a browser sign-in is pending.
    pub fn busy(&self, status: &EnrollmentStatus) -> bool {
        self.submitted
            || matches!(
                status,
                EnrollmentStatus::Authorizing { .. } | EnrollmentStatus::Saving
            )
    }

    /// End the connection editor and cancel sign-in according to its existing rules.
    pub fn close(
        &mut self,
        commands: &mpsc::Sender<SessionCommand>,
        status: &EnrollmentStatus,
    ) -> bool {
        if self.busy(status)
            && commands
                .try_send(SessionCommand::Enrollment(EnrollmentCommand::Cancel))
                .is_err()
        {
            self.notice = Some("The workspace is busy. Try closing Settings again.".into());
            return false;
        }
        self.open = false;
        if self.server_edited && !self.enrolled {
            self.server = self.saved_server.clone();
        }
        self.server_edited = false;
        self.label.clone_from(&self.saved_label);
        self.notice = None;
        true
    }

    /// Render connection fields, the browser action and its result.
    pub fn show_section(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mpsc::Sender<SessionCommand>,
        status: &EnrollmentStatus,
    ) {
        let context = ui.ctx().clone();
        let busy = self.busy(status);
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
        self.validation.request_repaint(&context, now);
        let mut action = None;
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
        });
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
    }
}
