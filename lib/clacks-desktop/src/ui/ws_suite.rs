//! WS Suite receiver configuration using the shared desktop settings controls.
use super::{
    feedback,
    fields::{self, FieldPresentation},
    validation::ValidationState,
};
use crate::{
    operating::{FieldError, issue},
    session::SessionCommand,
    ws_suite::{
        ReceiverAction, ReceiverRequest, ReceiverResponse, ReceiverView, ReceptionState,
        interfaces::InterfaceChoice,
    },
};
use clacks_client_store::ws_suite::{InterfaceSelection, ReceiverConnection, ReceiverSettings};
use eframe::egui;
use tokio::sync::{mpsc, oneshot};

/// Editable connection text, independent of active listener state.
#[derive(PartialEq)]
struct Form {
    /// Proposed reception enable flag.
    enabled: bool,
    /// Selected connection mode.
    multicast: bool,
    /// Last typed unicast address.
    local: String,
    /// Last typed multicast group; intentionally blank until supplied.
    group: String,
    /// Proposed UDP port.
    port: String,
    /// Stable selected interface name/address, not numeric OS index.
    interface: InterfaceSelection,
    /// Saved basis used for unapplied treatment and disabled Apply.
    basis: ReceiverSettings,
}

impl Form {
    /// Copy committed settings without inventing a multicast group.
    fn new(settings: ReceiverSettings) -> Self {
        let mut form = Self {
            enabled: settings.enabled,
            multicast: false,
            local: "127.0.0.1".into(),
            group: String::new(),
            port: "2237".into(),
            interface: InterfaceSelection::SystemDefault,
            basis: settings.clone(),
        };
        match settings.connection {
            ReceiverConnection::Unicast { address, port } => {
                form.local = address.to_string();
                form.port = port.to_string();
            }
            ReceiverConnection::Multicast {
                group,
                port,
                interface,
            } => {
                form.multicast = true;
                form.group = group.to_string();
                form.port = port.to_string();
                form.interface = interface;
            }
        }
        form
    }

    /// Build only active fields; Off retains the committed connection despite stale text.
    fn build(&self, interfaces: &[InterfaceChoice]) -> Result<ReceiverSettings, Vec<FieldError>> {
        if !self.enabled {
            return Ok(ReceiverSettings {
                enabled: false,
                connection: self.basis.connection.clone(),
            });
        }
        let label = if self.multicast {
            "Group address"
        } else {
            "Local address"
        };
        let address = if self.multicast {
            &self.group
        } else {
            &self.local
        };
        let mut errors = vec![];
        let address = address
            .trim()
            .parse::<std::net::IpAddr>()
            .map_err(|_| errors.push(issue(label, "Enter an IP address.")))
            .ok();
        let port = self
            .port
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0);
        if port.is_none() {
            errors.push(issue("Port", "Use a port from 1 to 65535."));
        }
        let (Some(address), Some(port)) = (address, port) else {
            return Err(errors);
        };
        let connection = if self.multicast {
            ReceiverConnection::Multicast {
                group: address,
                port,
                interface: self.interface.clone(),
            }
        } else {
            ReceiverConnection::Unicast { address, port }
        };
        let settings = ReceiverSettings {
            enabled: true,
            connection,
        };
        if let Err(error) = settings.validate() {
            errors.push(issue(
                if self.multicast && address.is_multicast() {
                    "Network interface"
                } else {
                    label
                },
                error,
            ));
        }
        if errors.is_empty()
            && let Err(error) = crate::ws_suite::socket::validate(&settings.connection, interfaces)
        {
            errors.push(issue(
                if self.multicast {
                    "Network interface"
                } else {
                    label
                },
                error,
            ));
        }
        if errors.is_empty() {
            Ok(settings)
        } else {
            Err(errors)
        }
    }
}

/// Receiver form and outcomes owned by the desktop window.
#[derive(Default)]
pub(super) struct ReceiverUi {
    /// Whether Settings has opened the receiver draft.
    open: bool,
    /// Pending input, absent until receiver settings load.
    form: Option<Form>,
    /// Current committed runtime view.
    view: ReceiverView,
    /// Current interface choices, refreshed on demand.
    interfaces: Vec<InterfaceChoice>,
    /// Pending receiver operation and its response, including across dismissal.
    pending: Option<oneshot::Receiver<ReceiverResponse>>,
    /// Submitted operation was an Apply rather than an enumeration.
    applying: bool,
    /// Restarting reception using the last committed receiver settings.
    retrying: bool,
    /// Independent OS enumeration feedback.
    notice: Option<String>,
    /// Current active-field errors.
    errors: Vec<FieldError>,
    /// Existing 300 ms validation state.
    validation: ValidationState,
    /// The settled Apply result, distinct from interface enumeration.
    applied: Option<Result<(), FieldError>>,
}

impl ReceiverUi {
    /// Expose the receiver draft to component tests.
    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open committed settings and enumerate current interfaces without blocking presentation.
    pub fn open(&mut self, view: &ReceiverView, commands: &mpsc::Sender<SessionCommand>) {
        if self.open {
            return;
        }
        self.open = true;
        self.view = view.clone();
        self.form = view.settings.clone().map(Form::new);
        self.validation.invalidate();
        if self.pending.is_none() {
            self.submit(ReceiverAction::Interfaces, commands);
        }
    }

    /// Submit one bounded operation and retain its response independently of modal visibility.
    fn submit(&mut self, action: ReceiverAction, commands: &mpsc::Sender<SessionCommand>) -> bool {
        let applying = matches!(action, ReceiverAction::Apply { .. });
        let (reply, result) = oneshot::channel();
        match commands.try_send(SessionCommand::WsSuite(ReceiverRequest { action, reply })) {
            Ok(()) => {
                self.pending = Some(result);
                self.applying = applying;
                self.notice = None;
                true
            }
            Err(_) => {
                self.notice = Some("The workspace is busy or closing.".into());
                false
            }
        }
    }

    /// Consume actual outcomes even after dismissal; never infer Apply success from a redraw.
    pub fn poll(&mut self, view: &ReceiverView) {
        self.view = view.clone();
        if self.form.is_none() {
            self.form = view.settings.clone().map(Form::new);
        }
        let result = self.pending.as_mut().map(oneshot::Receiver::try_recv);
        let Some(result) = result else { return };
        let response = match result {
            Ok(response) => response,
            Err(oneshot::error::TryRecvError::Empty) => return,
            Err(oneshot::error::TryRecvError::Closed) => ReceiverResponse::Failed(
                "The receiver request stopped unexpectedly. Try again.".into(),
            ),
        };
        self.pending = None;
        let retrying = std::mem::take(&mut self.retrying);
        let applying = std::mem::take(&mut self.applying) && !retrying;
        match response {
            ReceiverResponse::Interfaces(result) => match result {
                Ok(interfaces) => {
                    self.interfaces = interfaces;
                    self.validation.invalidate();
                }
                Err(error) => self.notice = Some(error),
            },
            ReceiverResponse::Applied(view) => {
                self.form = view.settings.clone().map(Form::new);
                if applying {
                    self.applied = Some(match &view.reception {
                        ReceptionState::Failed(error) => Err(issue("Reception", error.clone())),
                        _ => Ok(()),
                    });
                }
                self.view = view;
                self.validation.invalidate();
            }
            ReceiverResponse::Failed(error) => {
                if applying {
                    self.applied = Some(Err(issue("WS Suite", error)));
                } else {
                    self.notice = Some(error);
                }
            }
            ReceiverResponse::Closing => {
                if applying {
                    self.applied = Some(Err(issue("WS Suite", "The workspace is closing.")));
                }
            }
        }
    }

    /// Discard receiver form edits and leave Settings.
    pub fn close(&mut self) {
        self.open = false;
        self.form = None;
        self.errors.clear();
    }

    /// Whether receiver fields include any changes to submit or report.
    pub fn changed(&self) -> bool {
        self.form
            .as_ref()
            .is_some_and(|form| match form.build(&self.interfaces) {
                Ok(next) => next != form.basis,
                Err(_) => Form::new(form.basis.clone()) != *form,
            })
    }

    /// Whether the committed receiver configuration failed to activate.
    pub fn reception_failed(&self) -> bool {
        matches!(self.view.reception, ReceptionState::Failed(_))
    }

    /// Whether receiver work is already in progress.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Validate and send the receiver portion of the shared draft.
    pub fn apply(
        &mut self,
        commands: &mpsc::Sender<SessionCommand>,
    ) -> Result<(), Vec<FieldError>> {
        let Some(form) = &self.form else {
            return Err(vec![issue(
                "WS Suite",
                "Receiver settings are unavailable.",
            )]);
        };
        let next = form.build(&self.interfaces)?;
        if self.pending.is_some() || !self.submit(ReceiverAction::Apply { next }, commands) {
            return Err(vec![issue(
                "WS Suite",
                "Could not submit receiver settings.",
            )]);
        }
        Ok(())
    }

    /// Take the result of an explicit save, including any receiver activation failure.
    pub fn take_applied(&mut self) -> Option<Result<(), FieldError>> {
        self.applied.take()
    }

    /// Restore the current saved values after a shared Apply.
    pub fn reset_form(&mut self) {
        self.form = self.view.settings.clone().map(Form::new);
        self.errors.clear();
        self.validation.invalidate();
    }

    /// Show only actionable failures outside the modal, with direct settings access.
    pub fn feedback(&self, ui: &mut egui::Ui, available: bool) -> Option<egui::Id> {
        let failure = match &self.view.reception {
            ReceptionState::Failed(error) => Some(error.as_str()),
            _ => self.view.capture_notice.as_deref(),
        };
        failure.and_then(|failure| {
            feedback::problem(ui, failure);
            let button = ui.add_enabled(available, egui::Button::new("WS Suite settings…"));
            button.clicked().then_some(button.id)
        })
    }

    /// Render receiver settings and field diagnostics inside Settings.
    pub fn show_section(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mpsc::Sender<SessionCommand>,
        apply_errors: &[FieldError],
    ) {
        let context = ui.ctx().clone();
        let now = context.input(|input| input.time);
        if self.validation.due(now) {
            self.errors = self
                .form
                .as_ref()
                .map(|form| form.build(&self.interfaces).err().unwrap_or_default())
                .unwrap_or_default();
            self.validation.validated();
        }
        self.validation.request_repaint(&context, now);
        let busy = self.pending.is_some();
        let mut refresh_interfaces = false;
        let mut errors = self.errors.clone();
        errors.extend(
            apply_errors
                .iter()
                .map(|error| issue(&error.field, format!("⚠ {}", error.message))),
        );

        let Some(form) = &mut self.form else {
            ui.label("Loading receiver settings…");
            if let ReceptionState::Failed(error) = &self.view.reception {
                feedback::problem(ui, error);
            }
            return;
        };
        let mut edited = false;
        ui.add_enabled_ui(!busy, |ui| {
            if ui.checkbox(&mut form.enabled, "Receive WS Suite updates").changed() {
                edited = true;
                self.validation.invalidate();
            }
            ui.add_enabled_ui(form.enabled, |ui| {
                ui.horizontal(|ui| {
                    edited |= ui.selectable_value(&mut form.multicast, false, "Unicast").changed();
                    edited |= ui.selectable_value(&mut form.multicast, true, "Multicast").changed();
                });
                let basis = Form::new(form.basis.clone());
                let label = if form.multicast { "Group address" } else { "Local address" };
                let value = if form.multicast { &mut form.group } else { &mut form.local };
                let basis_address = if form.multicast { &basis.group } else { &basis.local };
                let changed = *value != *basis_address || form.multicast != basis.multicast;
                edited |= fields::text(
                    ui, egui::Id::unique("ws_suite.address"), label, value,
                    FieldPresentation {
                        edited: changed,
                        read_only: false,
                        errors: &errors,
                        validation_label: None,
                        radio_data: true,
                    },
                ).changed();
                let port_changed = form.port != basis.port;
                edited |= fields::text(
                    ui, egui::Id::unique("ws_suite.port"), "Port", &mut form.port,
                    FieldPresentation {
                        edited: port_changed,
                        read_only: false,
                        errors: &errors,
                        validation_label: None,
                        radio_data: true,
                    },
                ).changed();
                if form.multicast {
                    let interface_label = ui.label("Network interface");
                    let label = match &form.interface {
                        InterfaceSelection::SystemDefault => "System default".into(),
                        InterfaceSelection::Explicit { name, address } => {
                            let available = self.interfaces.iter().any(|entry| &entry.name == name && &entry.address == address);
                            format!("{name} · {address}{}", if available { "" } else { " (unavailable)" })
                        }
                    };
                    let before = form.interface.clone();
                    egui::ComboBox::from_id_salt("ws_suite.interface")
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.interface, InterfaceSelection::SystemDefault, "System default");
                            let family = form.group.parse::<std::net::IpAddr>().ok();
                            for entry in self.interfaces.iter().filter(|entry| family.is_none_or(|group| group.is_ipv4() == entry.address.is_ipv4())) {
                                ui.selectable_value(
                                    &mut form.interface,
                                    InterfaceSelection::Explicit { name: entry.name.clone(), address: entry.address },
                                    format!("{} · {}", entry.name, entry.address),
                                );
                            }
                        }).response.labelled_by(interface_label.id);
                    edited |= before != form.interface;
                    for error in errors.iter().filter(|error| error.field == "Network interface") {
                        ui.colored_label(ui.visuals().error_fg_color, &error.message);
                    }
                    if ui.button("Refresh interfaces").clicked() {
                        refresh_interfaces = true;
                    }
                    ui.small("Other applications can receive the same multicast updates. Enable capture in one Clacks client for each WS Suite source.");
                }
            });
        });
        if edited {
            self.validation.edited(now);
            if !form.enabled {
                self.errors.clear();
                self.validation.validated();
            }
        }
        let multicast = form.multicast;
        if let ReceptionState::Failed(error) = &self.view.reception {
            feedback::problem(ui, &format!("Reception unavailable. {error}"));
            let retry = ui.add_enabled(
                !busy && !self.changed(),
                egui::Button::new("Retry reception"),
            );
            if retry.clicked()
                && let Some(next) = self.view.settings.clone()
            {
                self.retrying = self.submit(ReceiverAction::Apply { next }, commands);
            }
        }
        if let Some(error) = &self.view.capture_notice {
            feedback::problem(ui, error);
        }
        for error in apply_errors
            .iter()
            .filter(|error| match error.field.as_str() {
                "Reception" | "Port" => false,
                "Local address" => multicast,
                "Group address" | "Network interface" => !multicast,
                _ => true,
            })
        {
            feedback::problem(ui, &format!("⚠ {}: {}", error.field, error.message));
        }
        if let Some(error) = &self.notice {
            feedback::problem(ui, error);
        }
        if refresh_interfaces {
            self.submit(ReceiverAction::Interfaces, commands);
        }
    }
}
