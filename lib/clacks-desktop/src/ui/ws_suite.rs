//! WS Suite receiver configuration using the shared desktop settings controls.
use super::{
    actions, dialog, feedback,
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

/// Dialog state and pending outcomes remain owned even while the dialog is dismissed.
#[derive(Default)]
pub(super) struct ReceiverUi {
    /// Visible settings modal.
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
    /// Independent OS enumeration feedback.
    notice: Option<String>,
    /// Current active-field errors.
    errors: Vec<FieldError>,
    /// Existing 300 ms validation state.
    validation: ValidationState,
    /// Focus destination after dismissal.
    opener: Option<egui::Id>,
    /// Deferred focus restoration after the modal leaves the frame.
    restore_focus: Option<egui::Id>,
}

impl ReceiverUi {
    /// Participate in shared single-modal admission.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open committed settings and enumerate current interfaces without blocking presentation.
    pub fn open(
        &mut self,
        opener: Option<egui::Id>,
        view: &ReceiverView,
        commands: &mpsc::Sender<SessionCommand>,
    ) {
        if self.open {
            return;
        }
        self.open = true;
        self.opener = opener;
        self.view = view.clone();
        self.form = view.settings.clone().map(Form::new);
        self.validation.invalidate();
        if self.pending.is_none() {
            self.submit(ReceiverAction::Interfaces, commands);
        }
    }

    /// Submit one bounded operation and retain its response independently of modal visibility.
    fn submit(&mut self, action: ReceiverAction, commands: &mpsc::Sender<SessionCommand>) {
        let applying = matches!(action, ReceiverAction::Apply { .. });
        let (reply, result) = oneshot::channel();
        match commands.try_send(SessionCommand::WsSuite(ReceiverRequest { action, reply })) {
            Ok(()) => {
                self.pending = Some(result);
                self.applying = applying;
                self.notice = None;
            }
            Err(_) => self.notice = Some("The workspace is busy or closing. Try again.".into()),
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
                let failed = matches!(view.reception, ReceptionState::Failed(_));
                self.view = view;
                if !failed {
                    self.dismiss();
                }
                self.validation.invalidate();
            }
            ReceiverResponse::Failed(error) => self.notice = Some(error),
            ReceiverResponse::Closing => self.notice = Some("The workspace is closing.".into()),
        }
    }

    /// Dismiss pending edits without undoing any submitted operation.
    fn dismiss(&mut self) {
        self.open = false;
        self.form = None;
        self.errors.clear();
        self.restore_focus = self.opener;
    }

    /// Show only actionable failures outside the modal, with direct settings access.
    pub fn feedback(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mpsc::Sender<SessionCommand>,
        available: bool,
    ) {
        if self.open {
            return;
        }
        let failure = match &self.view.reception {
            ReceptionState::Failed(error) => Some(error.as_str()),
            _ => self
                .notice
                .as_deref()
                .or(self.view.capture_notice.as_deref()),
        };
        if let Some(failure) = failure {
            feedback::problem(ui, failure);
            let button = ui.add_enabled(available, egui::Button::new("WS Suite settings…"));
            if button.clicked() {
                self.open(Some(button.id), &self.view.clone(), commands);
            }
        }
    }

    /// Render the existing shared settings chrome, field treatment and Apply/Cancel flow.
    pub fn show(&mut self, context: &egui::Context, commands: &mpsc::Sender<SessionCommand>) {
        if let Some(opener) = self.restore_focus.take() {
            context.memory_mut(|memory| memory.request_focus(opener));
        }
        if !self.open {
            return;
        }
        let now = context.input(|input| input.time);
        if self.validation.due(now) {
            self.errors = self
                .form
                .as_ref()
                .map(|form| form.build(&self.interfaces).err().unwrap_or_default())
                .unwrap_or_default();
            self.validation.validated();
        }
        self.validation.request_repaint(context, now);
        let busy = self.pending.is_some();
        let mut action = None;
        let mut cancel = false;
        let response = dialog::show(
            context,
            egui::Id::unique("ws_suite.dialog"),
            "WS Suite",
            |ui| {
                let Some(form) = &mut self.form else {
                    ui.label("Loading receiver settings…");
                    if let ReceptionState::Failed(error) = &self.view.reception {
                        feedback::problem(ui, error);
                    }
                    cancel = ui.button("Cancel").clicked();
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
                                errors: &self.errors,
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
                                errors: &self.errors,
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
                            for error in self.errors.iter().filter(|error| error.field == "Network interface") {
                                ui.colored_label(ui.visuals().error_fg_color, &error.message);
                            }
                            if ui.button("Refresh interfaces").clicked() {
                                action = Some(ReceiverAction::Interfaces);
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
                if let ReceptionState::Failed(error) = &self.view.reception {
                    feedback::problem(ui, &format!("Reception unavailable. {error}"));
                }
                if let Some(error) = &self.view.capture_notice {
                    feedback::problem(ui, error);
                }
                if let Some(error) = &self.notice {
                    feedback::problem(ui, error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if !self.errors.is_empty() {
                        let fields: std::collections::BTreeSet<_> = self
                            .errors
                            .iter()
                            .map(|error| error.field.as_str())
                            .collect();
                        ui.small(format!(
                            "Cannot apply: {}.",
                            fields.into_iter().collect::<Vec<_>>().join(", ")
                        ));
                    } else if form.build(&self.interfaces).ok().as_ref() != Some(&form.basis) {
                        ui.small("Not applied");
                    }
                    if actions::primary(
                        ui,
                        "Apply",
                        !busy && self.validation.can_submit(&self.errors),
                    )
                    .clicked()
                    {
                        match form.build(&self.interfaces) {
                            Ok(next) => action = Some(ReceiverAction::Apply { next }),
                            Err(errors) => self.errors = errors,
                        }
                    }
                    cancel = ui.button("Cancel").clicked();
                });
            },
        );
        if let Some(action) = action {
            self.submit(action, commands);
        }
        if cancel || response.cancel_requested {
            self.dismiss();
            context.request_repaint();
        }
    }
}
