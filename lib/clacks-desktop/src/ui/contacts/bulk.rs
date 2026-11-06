//! Non-modal selected-contact lookup feedback retained across history navigation.
use super::*;
use crate::directory::lookup::bulk::{
    BulkLookupHandle,
    report::{
        BulkContact, BulkCounts, BulkDetail, BulkDetailOutcome, BulkFailure, BulkReport, BulkStop,
        BulkUpdate, UnprocessedReason,
    },
};
use std::sync::Arc;

/// One admitted or pending menu action, separate from modal contact editing.
pub(super) struct BulkLookupUi {
    /// Original selection retained even if admission fails.
    ids: BTreeSet<QsoId>,
    /// Immediate admission acknowledgment, not the provider operation result.
    reply: Option<oneshot::Receiver<SettingsResponse>>,
    /// Tracked worker progress and independent cancellation.
    handle: Option<BulkLookupHandle>,
    /// Compact latest update or one shared final report.
    update: BulkUpdate,
    /// Unexpected channel failure without invented final accounting.
    error: Option<String>,
}

impl ContactsUi {
    /// Prevent duplicate lookup while admission or the selected consumer is active.
    pub fn bulk_running(&self) -> bool {
        self.bulk.as_ref().is_some_and(|bulk| {
            bulk.error.is_none() && !matches!(bulk.update, BulkUpdate::Finished(_))
        })
    }

    /// Admission depends on the actual stable selection, not visible history rows.
    pub fn can_lookup_directory(&self) -> bool {
        !self.selected.is_empty() && !self.selection_pending() && !self.bulk_running()
    }

    /// Submit one selection-based menu action without acquiring modal ownership.
    pub fn start_directory_lookup(&mut self, requests: &mpsc::Sender<SettingsRequest>) {
        if !self.can_lookup_directory() {
            return;
        }
        let ids = self.selected.clone();
        let (reply, receiver) = oneshot::channel();
        match requests.try_send(SettingsRequest {
            action: SettingsAction::Contacts(ContactAction::StartDirectoryLookup {
                ids: ids.clone(),
            }),
            reply,
        }) {
            Ok(()) => {
                self.bulk = Some(BulkLookupUi {
                    update: BulkUpdate::Preparing {
                        total: ids.len() as u64,
                    },
                    ids,
                    reply: Some(receiver),
                    handle: None,
                    error: None,
                })
            }
            Err(error) => {
                self.problem = true;
                self.notice = Some(format!("Could not start directory lookup: {error}"));
            }
        }
    }

    /// Poll independent admission/progress even when native menu state updates without painting.
    pub fn poll_bulk_lookup(&mut self) {
        let Some(bulk) = &mut self.bulk else {
            return;
        };
        if let Some(reply) = &mut bulk.reply {
            match reply.try_recv() {
                Ok(SettingsResponse::Contacts(ContactResponse::DirectoryLookupStarted(handle))) => {
                    bulk.handle = Some(handle);
                    bulk.reply = None;
                }
                Ok(response) => {
                    let message = match response {
                        SettingsResponse::Failed(message) => message,
                        SettingsResponse::Closing => {
                            "The workspace is closing; lookup did not start.".into()
                        }
                        _ => "Directory lookup admission ended without a usable response.".into(),
                    };
                    let failure = BulkFailure::Configuration {
                        message: message.clone(),
                    };
                    bulk.update = BulkUpdate::Finished(Arc::new(BulkReport {
                        total: bulk.ids.len() as u64,
                        counts: BulkCounts {
                            unprocessed: bulk.ids.len() as u64,
                            ..Default::default()
                        },
                        details: bulk
                            .ids
                            .iter()
                            .map(|id| BulkDetail {
                                contact: BulkContact {
                                    id: *id,
                                    callsign: None,
                                    start: None,
                                },
                                outcome: BulkDetailOutcome::Unprocessed {
                                    reason: UnprocessedReason::Stopped {
                                        failure: failure.clone(),
                                    },
                                },
                            })
                            .collect(),
                        stop: BulkStop::PreparationFailed { message },
                    }));
                    bulk.reply = None;
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    bulk.error = Some("Lookup admission ended without acknowledgment. Check workspace diagnostics before retrying.".into());
                    bulk.reply = None;
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
        }
        if let Some(handle) = &mut bulk.handle {
            bulk.update = handle.updates.borrow_and_update().clone();
            if handle.updates.has_changed().is_err()
                && !matches!(bulk.update, BulkUpdate::Finished(_))
            {
                bulk.error = Some("Directory lookup stopped without a final report. Completed updates remain saved; check workspace diagnostics.".into());
            }
        }
    }

    /// Draw compact progress or a retained partial report in the history tool.
    pub fn bulk_feedback(&mut self, ui: &mut egui::Ui) {
        let Some(bulk) = &self.bulk else {
            return;
        };
        let mut dismiss = false;
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.strong("Directory lookup");
            if matches!(bulk.update, BulkUpdate::Finished(_)) || bulk.error.is_some() {
                dismiss = ui.small_button("Dismiss lookup report").clicked();
            } else if let Some(handle) = &bulk.handle {
                let cancelling = handle.cancel.is_cancelled();
                if ui
                    .add_enabled(!cancelling, egui::Button::new("Cancel lookup"))
                    .clicked()
                {
                    handle.cancel.cancel();
                }
                if cancelling {
                    ui.small("Cancelling…");
                }
            }
        });
        if let Some(error) = &bulk.error {
            feedback::problem(ui, error);
        }
        match &bulk.update {
            BulkUpdate::Preparing { total } => {
                ui.label(format!("Preparing {total} selected contacts…"));
            }
            BulkUpdate::Running { total, counts } => {
                ui.label(format!("{} of {total} contacts", counts.settled()));
                ui.add(egui::ProgressBar::new(if *total == 0 {
                    0.0
                } else {
                    counts.settled() as f32 / *total as f32
                }));
            }
            BulkUpdate::Finished(report) => show_report(ui, report),
        }
        if !matches!(bulk.update, BulkUpdate::Finished(_)) && bulk.error.is_none() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        if dismiss {
            self.bulk = None;
        }
    }

    /// Reconcile a queued native command with the latest actual selected logbook.
    pub fn reconcile_view(&mut self, view: SelectedView) {
        if self.query.view != view {
            self.refresh(view);
        }
    }
}

/// Render application-level outcomes without confusing provider results with committed writes.
fn show_report(ui: &mut egui::Ui, report: &BulkReport) {
    match &report.stop {
        BulkStop::Completed => {
            ui.label(format!("Completed · {} selected contacts", report.total));
        }
        BulkStop::Cancelled => {
            ui.label("Cancelled. Completed updates were kept.");
        }
        BulkStop::PreparationCancelled => {
            ui.label("Cancelled during preparation. Lookup did not start; no bulk snapshot writes were made.");
        }
        BulkStop::Stopped { reason } => {
            feedback::problem(ui, &reason.to_string());
            ui.label(reason.guidance());
        }
        BulkStop::PreparationFailed { message } => {
            feedback::problem(ui, message);
            ui.label("Lookup did not start; no bulk snapshot writes were made.");
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("New/updated: {}", report.counts.new_updated));
        ui.label(format!("Unchanged: {}", report.counts.unchanged));
        ui.label(format!("Not found: {}", report.counts.not_found));
        ui.label(format!("Failed: {}", report.counts.failed));
        ui.label(format!("Unprocessed: {}", report.counts.unprocessed));
    });
    for failed in [true, false] {
        let count = if failed {
            report.counts.failed
        } else {
            report.counts.unprocessed
        };
        if count == 0 {
            continue;
        }
        let label = if failed {
            format!("Failed contacts ({count})")
        } else {
            format!("Unprocessed contacts ({count})")
        };
        egui::CollapsingHeader::new(label)
            .id_salt(("directory.bulk.details", failed))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(160.0)
                    .id_salt(("directory.bulk.rows", failed))
                    .show(ui, |ui| {
                        for detail in &report.details {
                            if matches!(detail.outcome, BulkDetailOutcome::Failed { .. }) != failed
                            {
                                continue;
                            }
                            ui.horizontal_wrapped(|ui| {
                                if let Some(call) = &detail.contact.callsign {
                                    ui.strong(call.as_str());
                                } else {
                                    ui.strong("Contact details unavailable");
                                }
                                if let Some(start) = detail.contact.start {
                                    ui.label(format!(
                                        "{} UTC",
                                        crate::logging::time::format(start)
                                    ));
                                }
                            });
                            match &detail.outcome {
                                BulkDetailOutcome::Failed { failure } => {
                                    ui.add(egui::Label::new(failure.to_string()).wrap());
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(failure.guidance()).small(),
                                        )
                                        .wrap(),
                                    );
                                }
                                BulkDetailOutcome::Unprocessed { reason } => {
                                    ui.add(egui::Label::new(reason.to_string()).wrap());
                                }
                            }
                            ui.small(format!("Contact ID: {}", detail.contact.id));
                            ui.add_space(4.0);
                        }
                    });
            });
    }
}
