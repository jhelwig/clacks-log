//! Export setup, native destination selection and retained results.

mod report;
mod setup;
#[cfg(test)]
mod tests;

use self::report::{show_files, show_result};
use super::feedback;
use crate::{
    export::{
        ExportFormat, PreparedExport,
        publish::{ExportDestination, ExportReport, PlannedFileName, PublishError},
    },
    session::{
        SessionCommand,
        export::{Action, Capture, History, Reply, Request, Scope},
    },
};
use clacks_model::{
    export::select::ContextResolution,
    ids::{AccountId, QsoId},
};
use eframe::egui;
use futures_util::future::BoxFuture;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(super) struct ExportUi {
    active: Option<ActiveExport>,
    picker: Option<BoxFuture<'static, Option<PathBuf>>>,
}

struct ActiveExport {
    account: AccountId,
    /// Publication can finish after the workspace stops accepting new writes.
    retired: bool,
    attempt: u64,
    opener: Option<egui::Id>,
    history: Arc<History>,
    applied: String,
    view_label: String,
    scope: Scope,
    format: ExportFormat,
    capture: Option<Arc<Capture>>,
    plan: Option<Arc<PreparedExport>>,
    resolutions: BTreeMap<QsoId, ContextResolution>,
    phase: Phase,
    names: Vec<PlannedFileName>,
    stop: CancellationToken,
    pending: Option<oneshot::Receiver<Result<Reply, String>>>,
    problem: Option<String>,
}

enum Phase {
    Setup,
    Files,
    Replace(PathBuf),
    Writing(ExportDestination),
    Result {
        destination: ExportDestination,
        outcome: Option<Result<ExportReport, PublishError>>,
    },
}

struct PickerWake(egui::Context);

impl Wake for PickerWake {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint_of(egui::ViewportId::ROOT);
    }
}

impl ExportUi {
    pub(super) fn is_open(&self) -> bool {
        self.active.is_some() || self.is_picking_file()
    }

    pub(super) fn is_picking_file(&self) -> bool {
        self.picker.is_some()
    }

    pub(super) fn open(
        &mut self,
        account: AccountId,
        history: History,
        applied: String,
        view_label: String,
        opener: Option<egui::Id>,
        commands: &mpsc::Sender<SessionCommand>,
    ) {
        static NEXT_ATTEMPT: AtomicU64 = AtomicU64::new(1);
        let scope = if history.selected.is_empty() {
            Scope::Matching
        } else {
            Scope::Selected
        };
        let mut active = ActiveExport {
            account,
            retired: false,
            attempt: NEXT_ATTEMPT.fetch_add(1, Ordering::Relaxed),
            opener,
            history: Arc::new(history),
            applied,
            view_label,
            scope,
            format: ExportFormat::Ordinary,
            capture: None,
            plan: None,
            resolutions: BTreeMap::new(),
            phase: Phase::Setup,
            names: Vec::new(),
            stop: CancellationToken::new(),
            pending: None,
            problem: None,
        };
        active.prepare(commands, false);
        self.active = Some(active);
    }

    /// A native picker must be allowed to settle even if its workspace retired.
    pub(super) fn poll(
        &mut self,
        context: &egui::Context,
        commands: &mpsc::Sender<SessionCommand>,
        account: Option<AccountId>,
    ) {
        if let Some(active) = &mut self.active
            && (Some(active.account) != account
                || active
                    .capture
                    .as_ref()
                    .is_some_and(|capture| capture.workspace_stop.is_cancelled()))
        {
            active.stop.cancel();
            match active.phase {
                Phase::Writing(_) | Phase::Result { .. } => active.retired = true,
                _ => self.active = None,
            }
        }
        if let Some(picker) = &mut self.picker {
            let waker = Waker::from(Arc::new(PickerWake(context.clone())));
            if let Poll::Ready(path) = picker.as_mut().poll(&mut Context::from_waker(&waker)) {
                self.picker = None;
                context.send_viewport_cmd(egui::ViewportCommand::Focus);
                if let (Some(path), Some(active)) = (path, &mut self.active)
                    && !active.retired
                {
                    let destination = match active.format {
                        ExportFormat::Ordinary => ExportDestination::Ordinary {
                            path,
                            replace: false,
                        },
                        ExportFormat::Pota => ExportDestination::Pota {
                            directory: path,
                            names: active.names.clone(),
                        },
                    };
                    active.publish(commands, destination);
                }
            }
        }
        if let Some(active) = &mut self.active {
            active.poll();
        }
    }

    pub(super) fn show(
        &mut self,
        context: &egui::Context,
        commands: &mpsc::Sender<SessionCommand>,
        parent: &eframe::Frame,
    ) {
        let Some(active) = &mut self.active else {
            return;
        };
        let mut dismiss = false;
        egui::Window::new("Export ADI")
            .id(egui::Id::unique("adi_export"))
            .default_width(510.0)
            .collapsible(false)
            .vscroll(true)
            .show(context, |ui| {
                ui.set_max_width(600.0);
                if let Some(problem) = &active.problem {
                    feedback::problem(ui, problem);
                }
                if self.picker.is_some() {
                    ui.label("Choose the destination in the system dialog.");
                    return;
                }
                match &active.phase {
                    Phase::Setup => {
                        dismiss = active.show_setup(ui, commands, parent, &mut self.picker)
                    }
                    Phase::Files => {
                        show_files(ui, &active.names);
                        ui.horizontal(|ui| {
                            if ui.button("Back").clicked() {
                                active.phase = Phase::Setup;
                                active.problem = None;
                            }
                            if ui.button("Choose folder…").clicked() {
                                active.open_picker(ui.ctx(), parent, &mut self.picker);
                            }
                        });
                    }
                    Phase::Replace(path) => {
                        let path = path.clone();
                        ui.label(format!("Replace the existing file {}?", path.display()));
                        ui.horizontal(|ui| {
                            if ui.button("Back").clicked() {
                                active.phase = Phase::Setup;
                                active.problem = None;
                            }
                            if ui.button("Choose another name…").clicked() {
                                active.open_picker(ui.ctx(), parent, &mut self.picker);
                            }
                            if ui.button("Replace file").clicked() {
                                active.publish(
                                    commands,
                                    ExportDestination::Ordinary {
                                        path,
                                        replace: true,
                                    },
                                );
                            }
                        });
                    }
                    Phase::Writing(_) => {
                        let stopping = active.stop.is_cancelled();
                        ui.spinner();
                        ui.label(if stopping {
                            "Stopping export…"
                        } else {
                            "Writing export files…"
                        });
                        if ui
                            .add_enabled(!stopping, egui::Button::new("Cancel export"))
                            .clicked()
                        {
                            active.stop.cancel();
                        }
                    }
                    Phase::Result {
                        destination,
                        outcome,
                    } => {
                        show_result(
                            ui,
                            destination,
                            outcome.as_ref(),
                            active.plan.as_deref(),
                            active.capture.as_deref(),
                        );
                        let retry_folder = !active.retired
                            && matches!(destination, ExportDestination::Pota { .. })
                            && matches!(outcome, Some(Err(error)) if error.is_collision());
                        ui.horizontal(|ui| {
                            dismiss = ui.button("Done").clicked();
                            if retry_folder && ui.button("Choose another folder…").clicked() {
                                active.open_picker(ui.ctx(), parent, &mut self.picker);
                            }
                        });
                    }
                }
            });
        if dismiss {
            active.stop.cancel();
            if let Some(opener) = active.opener {
                context.memory_mut(|memory| memory.request_focus(opener));
            }
            self.active = None;
            context.request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}

impl ActiveExport {
    fn request(&mut self, commands: &mpsc::Sender<SessionCommand>, action: Action) -> bool {
        self.stop.cancel();
        self.stop = CancellationToken::new();
        self.pending = None;
        self.problem = None;
        let (reply, receiver) = oneshot::channel();
        let request = Request {
            account: self.account,
            attempt: self.attempt,
            stop: self.stop.clone(),
            action,
            reply,
        };
        if commands.try_send(SessionCommand::Export(request)).is_ok() {
            self.pending = Some(receiver);
            true
        } else {
            self.problem = Some("The workspace is busy. Try the export again.".into());
            false
        }
    }

    fn prepare(&mut self, commands: &mpsc::Sender<SessionCommand>, reuse_capture: bool) {
        self.plan = None;
        if !reuse_capture {
            self.capture = None;
            self.resolutions.clear();
        }
        self.request(
            commands,
            Action::Prepare {
                history: self.history.clone(),
                scope: self.scope,
                format: self.format,
                capture: self.capture.clone(),
                resolutions: self.resolutions.clone(),
            },
        );
    }

    fn publish(&mut self, commands: &mpsc::Sender<SessionCommand>, destination: ExportDestination) {
        if let (Some(plan), Some(capture)) = (&self.plan, &self.capture)
            && self.request(
                commands,
                Action::Publish {
                    plan: plan.clone(),
                    destination: destination.clone(),
                    captured_workspace_stop: capture.workspace_stop.clone(),
                },
            )
        {
            self.phase = Phase::Writing(destination);
        }
    }

    fn poll(&mut self) {
        let Some(receiver) = &mut self.pending else {
            return;
        };
        let completion = match receiver.try_recv() {
            Ok(completion) => completion,
            Err(oneshot::error::TryRecvError::Empty) => return,
            Err(oneshot::error::TryRecvError::Closed) => {
                Err("The export operation ended without a result.".into())
            }
        };
        self.pending = None;
        match completion {
            Ok(Reply::Prepared { capture, plan }) => {
                self.capture = Some(capture);
                self.plan = Some(plan);
            }
            Ok(Reply::Published(outcome)) => {
                if let Phase::Writing(destination) = &self.phase {
                    self.phase = match (destination, &outcome) {
                        (
                            ExportDestination::Ordinary {
                                path,
                                replace: false,
                            },
                            Err(error),
                        ) if !self.retired && error.is_collision() => Phase::Replace(path.clone()),
                        _ => Phase::Result {
                            destination: destination.clone(),
                            outcome: Some(outcome),
                        },
                    };
                }
            }
            Err(problem) => {
                self.problem = Some(problem);
                if let Phase::Writing(destination) = &self.phase {
                    self.phase = Phase::Result {
                        destination: destination.clone(),
                        outcome: None,
                    };
                }
            }
        }
    }

    fn open_picker(
        &mut self,
        context: &egui::Context,
        parent: &eframe::Frame,
        picker: &mut Option<BoxFuture<'static, Option<PathBuf>>>,
    ) {
        if parent.window_handle().is_err() || parent.display_handle().is_err() {
            self.problem =
                Some("The Clacks window is unavailable for destination selection.".into());
            return;
        }
        let dialog = rfd::AsyncFileDialog::new().set_parent(parent);
        *picker = Some(match self.format {
            ExportFormat::Ordinary => {
                let selection = dialog
                    .set_title("Save ADI file")
                    .set_file_name("contacts.adi")
                    .add_filter("ADIF text", &["adi", "adif"])
                    .save_file();
                Box::pin(async move { selection.await.map(|file| file.path().to_owned()) })
            }
            ExportFormat::Pota => {
                let selection = dialog
                    .set_title("Choose a folder for POTA files")
                    .set_can_create_directories(true)
                    .pick_folder();
                Box::pin(async move { selection.await.map(|file| file.path().to_owned()) })
            }
        });
        self.problem = None;
        context.request_repaint_of(egui::ViewportId::ROOT);
    }
}
