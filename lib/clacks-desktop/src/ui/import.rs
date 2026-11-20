//! Desktop import workflow from native file selection to the final row report.

mod review;

use super::{dialog, feedback};
use crate::{
    import::workflow::{PrepareRequest, Report, ReviewSession, RowReadiness, tags},
    session::SessionCommand,
};
use clacks_client_store::{logbooks::selection::SelectedView, settings::SettingsSnapshot};
use clacks_model::ids::TagId;
use eframe::egui;
use futures_util::future::BoxFuture;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    path::PathBuf,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(super) struct ImportUi {
    open: bool,
    opener: Option<egui::Id>,
    stop: CancellationToken,
    tag: Option<TagId>,
    path: Option<PathBuf>,
    file: Option<BoxFuture<'static, Option<PathBuf>>>,
    review: Option<Box<ReviewSession>>,
    prepared: Option<oneshot::Receiver<ReviewSession>>,
    applied: Option<oneshot::Receiver<Result<Report, String>>>,
    progress: Option<watch::Receiver<usize>>,
    apply_total: usize,
    report: Option<Report>,
    applying: bool,
    error: Option<String>,
    selected: usize,
}

/// Schedule a root viewport repaint when the native picker resolves.
struct PickerWake(egui::Context);

impl Wake for PickerWake {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint_of(egui::ViewportId::ROOT);
    }
}

impl ImportUi {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_picking_file(&self) -> bool {
        self.file.is_some()
    }

    pub fn poll_picker(&mut self, context: &egui::Context) {
        let Some(picker) = &mut self.file else {
            return;
        };
        let waker = Waker::from(Arc::new(PickerWake(context.clone())));
        if let Poll::Ready(path) = picker.as_mut().poll(&mut Context::from_waker(&waker)) {
            self.file = None;
            if let Some(path) = path {
                self.path = Some(path);
            }
        }
    }

    pub fn open(&mut self, settings: &SettingsSnapshot, opener: Option<egui::Id>) {
        let available = tags(settings);
        let tag = match settings.logbooks.selected_view {
            SelectedView::Logbook(tag) if available.contains_key(&tag) => Some(tag),
            _ if available.len() == 1 => available.keys().next().copied(),
            _ => None,
        };
        *self = Self {
            open: true,
            opener,
            tag,
            ..Self::default()
        };
    }

    pub fn show(
        &mut self,
        context: &egui::Context,
        commands: &mpsc::Sender<SessionCommand>,
        settings: Option<&SettingsSnapshot>,
        parent: &eframe::Frame,
    ) {
        if !self.open {
            return;
        }
        self.poll();
        let response = dialog::show_with_dismissal(
            context,
            egui::Id::unique("adi_import"),
            "Import ADI",
            !self.is_picking_file(),
            |ui| {
                if let Some(error) = &self.error {
                    feedback::problem(ui, error);
                }
                if self.is_picking_file() {
                    ui.label("Choose a file in the file picker.");
                    false
                } else if let Some(report) = &self.report {
                    show_report(ui, report);
                    ui.button("Done").clicked()
                } else if self.applying {
                    let stopping = self.stop.is_cancelled();
                    ui.label(if stopping {
                        "Stopping after the current contact…"
                    } else {
                        "Applying reviewed contacts…"
                    });
                    if let Some(progress) = &self.progress {
                        let settled = *progress.borrow();
                        ui.label(format!("{settled} of {} rows processed", self.apply_total));
                    }
                    if ui
                        .add_enabled(!stopping, egui::Button::new("Cancel import"))
                        .clicked()
                    {
                        self.stop.cancel();
                    }
                    false
                } else if self.review.is_some() {
                    self.show_review(ui)
                } else if self.applied.is_some() {
                    ui.label("Preparing import…");
                    ui.button("Cancel").clicked()
                } else if let Some(settings) = settings {
                    self.show_source(ui, commands, settings, parent)
                } else {
                    ui.label("The workspace is opening.");
                    ui.button("Cancel").clicked()
                }
            },
        );
        if !self.is_picking_file() && (response.cancel_requested || response.inner) {
            self.stop.cancel();
            if !self.applying {
                if let Some(opener) = self.opener {
                    context.memory_mut(|memory| memory.request_focus(opener));
                }
                *self = Self::default();
                context.request_repaint_of(egui::ViewportId::ROOT);
            }
        }
    }

    fn poll(&mut self) {
        use oneshot::error::TryRecvError;
        if let Some(receiver) = &mut self.prepared {
            match receiver.try_recv() {
                Ok(review) => {
                    self.review = Some(Box::new(review));
                    self.prepared = None;
                }
                Err(TryRecvError::Closed) => self.prepared = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(receiver) = &mut self.applied {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Closed) => Some(Err("The import did not finish. Reopen the source to review current saved contacts.".into())),
                Err(TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.applied = None;
                self.prepared = None;
                self.review = None;
                self.applying = false;
                match result {
                    Ok(report) => self.report = Some(report),
                    Err(error) => self.error = Some(error),
                }
            }
        }
    }

    fn show_source(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mpsc::Sender<SessionCommand>,
        settings: &SettingsSnapshot,
        parent: &eframe::Frame,
    ) -> bool {
        let available = tags(settings);
        if self.tag.is_some_and(|tag| !available.contains_key(&tag)) {
            self.tag = None;
        }
        if available.is_empty() {
            ui.label("Create a named logbook before importing contacts.");
        }
        ui.horizontal(|ui| {
            let label = ui.label("Destination logbook");
            let selected = self
                .tag
                .and_then(|tag| available.get(&tag))
                .map_or("Choose a logbook", String::as_str);
            egui::ComboBox::from_id_salt("import_destination")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (tag, name) in &available {
                        let label = if available.values().filter(|value| *value == name).count() > 1
                        {
                            format!("{name} · {tag}")
                        } else {
                            name.clone()
                        };
                        ui.selectable_value(&mut self.tag, Some(*tag), label);
                    }
                })
                .response
                .labelled_by(label.id);
        });
        ui.horizontal_wrapped(|ui| {
            let display = self
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "No file selected".into());
            ui.label(display);
            if ui.button("Choose file…").clicked() {
                if parent.window_handle().is_ok() && parent.display_handle().is_ok() {
                    // The macOS backend creates its sheet when pick_file is called.
                    let picker = rfd::AsyncFileDialog::new()
                        .set_parent(parent)
                        .set_title("Choose an ADI logbook")
                        .add_filter("ADIF text", &["adi", "adif"])
                        .pick_file();
                    self.file = Some(Box::pin(async move {
                        picker.await.map(|file| file.path().to_owned())
                    }));
                    self.error = None;
                    ui.ctx().request_repaint();
                } else {
                    self.error =
                        Some("The Clacks window is unavailable for file selection.".into());
                }
            }
        });
        let mut close = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.file.is_none() && self.path.is_some() && self.tag.is_some(),
                    egui::Button::new("Prepare import"),
                )
                .clicked()
                && let (Some(path), Some(tag)) = (self.path.clone(), self.tag)
            {
                let (review, prepared) = oneshot::channel();
                let (completion, applied) = oneshot::channel();
                let (progress, row_progress) = watch::channel(0);
                let request = PrepareRequest {
                    path,
                    tag,
                    stop: self.stop.clone(),
                    review,
                    completion,
                    progress,
                };
                match commands.try_send(SessionCommand::Import(request)) {
                    Ok(()) => {
                        self.prepared = Some(prepared);
                        self.applied = Some(applied);
                        self.progress = Some(row_progress);
                        self.error = None;
                    }
                    Err(_) => self.error = Some("The workspace is busy. Try again.".into()),
                }
            }
            close = ui
                .add_enabled(!self.is_picking_file(), egui::Button::new("Cancel"))
                .clicked();
        });
        close
    }

    fn show_review(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(session) = &mut self.review else {
            return false;
        };
        let prepared = &mut session.prepared;
        ui.label(format!("{} → {}", prepared.file, prepared.destination));
        ui.label(format!("{} readable contacts", prepared.rows.len()));
        ui.label("Sync is paused while this import is open.");
        if let Some(program) = &prepared.metadata.program {
            ui.label(format!(
                "Source {} {}",
                program,
                prepared.metadata.program_version.as_deref().unwrap_or("")
            ));
        }
        for diagnostic in &prepared.header_warnings {
            feedback::problem(
                ui,
                &format!(
                    "Header byte {}: {}",
                    diagnostic.span.start, diagnostic.message
                ),
            );
        }
        if let Some(error) = &prepared.terminal {
            feedback::problem(
                ui,
                &format!(
                    "Preparation stopped at byte {}. {}. The rest of the file will remain unprocessed.",
                    error.span.start, error.message
                ),
            );
        }
        show_unsupported(ui, &prepared.unsupported);
        ui.collapsing("Source row warnings", |ui| {
            egui::ScrollArea::vertical()
                .id_salt("adi_prepared_warnings")
                .max_height(130.0)
                .show(ui, |ui| {
                    let mut any = false;
                    for (row, review) in prepared.rows.iter().enumerate() {
                        let source = &review.source;
                        for warning in &source.warnings {
                            any = true;
                            if ui
                                .link(format!(
                                    "Row {} · Byte {} · {}",
                                    source.number, warning.span.start, warning.message
                                ))
                                .clicked()
                            {
                                self.selected = row;
                            }
                        }
                    }
                    if !any {
                        ui.label("No row warnings");
                    }
                });
        });
        ui.collapsing("Rows that will not be applied", |ui| {
            egui::ScrollArea::vertical()
                .id_salt("adi_prepared_skips")
                .max_height(130.0)
                .show(ui, |ui| {
                    let mut any = false;
                    for (row, review) in prepared.rows.iter().enumerate() {
                        let source = &review.source;
                        let reason = match prepared.readiness(row) {
                            Ok(RowReadiness::NeedsAssignment) => {
                                "Source or match choices required".into()
                            }
                            Ok(RowReadiness::Dropped) => "Dropped during review".into(),
                            Ok(RowReadiness::NeedsFields) => "Field choices required".into(),
                            Ok(RowReadiness::Ready { .. }) => continue,
                            Err(error) => error.to_string(),
                        };
                        any = true;
                        if ui
                            .link(format!("Row {} · {reason}", source.number))
                            .clicked()
                        {
                            self.selected = row;
                        }
                    }
                    if !any {
                        ui.label("Every row is ready to apply");
                    }
                });
        });
        if !prepared.rows.is_empty() {
            ui.separator();
            let height = ui.text_style_height(&egui::TextStyle::Button);
            egui::ScrollArea::vertical()
                .id_salt("adi_rows")
                .max_height(170.0)
                .show_rows(ui, height, prepared.rows.len(), |ui, range| {
                    for (row, review) in prepared
                        .rows
                        .iter()
                        .enumerate()
                        .skip(range.start)
                        .take(range.len())
                    {
                        let number = review.source.number;
                        let name = review
                            .worked_call()
                            .map_or("Missing callsign", |call| call.as_str());
                        let state = match prepared.readiness(row) {
                            Ok(RowReadiness::NeedsFields) => "Review fields",
                            Ok(RowReadiness::Ready { target: None, .. }) => "Create",
                            Ok(RowReadiness::Ready {
                                target: Some(_), ..
                            }) => "Reuse saved QSO",
                            Ok(RowReadiness::Dropped) => "Drop",
                            Ok(RowReadiness::NeedsAssignment) | Err(_) => "Review required",
                        };
                        ui.selectable_value(
                            &mut self.selected,
                            row,
                            format!("Row {number} · {name} · {state}"),
                        );
                    }
                });
            ui.separator();
            review::review_row(ui, prepared, self.selected);
        }
        ui.separator();
        let mut apply = false;
        let mut close = false;
        ui.horizontal(|ui| {
            apply = ui
                .add_enabled(
                    !prepared.rows.is_empty(),
                    egui::Button::new("Apply reviewed contacts"),
                )
                .clicked();
            close = ui.button("Cancel import").clicked();
        });
        ui.label("Rows that still need review will remain unapplied.");
        if apply && let Some(session) = self.review.take() {
            self.apply_total = session.prepared.rows.len();
            self.applying = session.apply.send(session.prepared).is_ok();
        }
        close
    }
}

fn show_report(ui: &mut egui::Ui, report: &Report) {
    ui.heading(if report.cancelled {
        "Import cancelled"
    } else {
        "Import complete"
    });
    egui::Grid::new("adi_report").show(ui, |ui| {
        for (label, count) in [
            ("QSOs created", report.created),
            ("QSOs updated", report.changed),
            ("Logbook memberships added", report.memberships),
            ("Service assertions added", report.assertions),
            ("Reused QSOs with unchanged contact facts", report.unchanged),
            ("Dropped rows", report.dropped),
            (
                "Warnings",
                report.header_warnings.len() + report.warnings.len(),
            ),
            ("Failed rows", report.failures.len()),
            ("Unprocessed readable rows", report.unprocessed),
        ] {
            ui.label(label);
            ui.label(count.to_string());
            ui.end_row();
        }
    });
    if let Some(error) = &report.terminal {
        feedback::problem(
            ui,
            &format!(
                "Byte {}: {}. The unreliable remainder was left unprocessed.",
                error.span.start, error.message
            ),
        );
    }
    show_unsupported(ui, &report.unsupported);
    for diagnostic in &report.header_warnings {
        feedback::problem(
            ui,
            &format!(
                "Header byte {}: {}",
                diagnostic.span.start, diagnostic.message
            ),
        );
    }
    if !report.warnings.is_empty() {
        ui.collapsing("Row warnings", |ui| {
            for (row, diagnostic) in &report.warnings {
                ui.label(format!("Row {row}: {}", diagnostic.message));
            }
        });
    }
    if !report.skipped.is_empty() {
        ui.collapsing("Dropped and unprocessed rows", |ui| {
            for (row, reason) in &report.skipped {
                ui.label(format!("Row {row}: {reason}"));
            }
        });
    }
    for (row, reason) in &report.failures {
        feedback::problem(ui, &format!("Row {row}: {reason}"));
    }
}

fn show_unsupported(ui: &mut egui::Ui, items: &std::collections::BTreeMap<String, usize>) {
    if !items.is_empty() {
        ui.collapsing("Unsupported source fields", |ui| {
            for (field, count) in items {
                ui.label(format!("{field} · {count} contacts"));
            }
        });
    }
}

#[cfg(test)]
#[test]
fn import_cannot_be_dismissed_until_its_file_picker_closes() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };

    let (finish, picker) = oneshot::channel();
    let (commands, _received) = mpsc::channel(1);
    let mut ui = Harness::builder().build_ui_state(
        |ui, import: &mut ImportUi| {
            import.poll_picker(ui.ctx());
            import.show(ui.ctx(), &commands, None, &eframe::Frame::_new_kittest());
        },
        ImportUi {
            open: true,
            file: Some(Box::pin(async move { picker.await.ok().flatten() })),
            ..Default::default()
        },
    );
    ui.key_press(egui::Key::Escape);
    ui.run();
    assert!(ui.state().is_open());
    assert!(ui.get_by_label("×").accesskit_node().is_disabled());

    assert!(finish.send(None).is_ok());
    ui.run();
    assert!(ui.state().is_open());
    ui.key_press(egui::Key::Escape);
    ui.run();
    assert!(!ui.state().is_open());
}
