//! Workspace session lifetime, independent of native redraws.

mod opening;
pub(crate) mod settings;
mod shutdown;
#[cfg(test)]
pub(crate) mod tests;

use crate::{
    application::STARTUP_DURATION_METRIC,
    enrollment::{EnrollmentCommand, EnrollmentStatus, worker},
    instance::activation::ActivationListener,
    telemetry::TelemetryGuard,
    workspace::{OpenedWorkspace, WorkspacePaths},
};
use clacks_client_store::{identity::WorkspaceStatus, settings::SettingsSnapshot};
use eframe::egui;
use std::{
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::instrument::WithSubscriber;

/// Bound blocking font discovery/loading so rapid preference changes cannot grow workers indefinitely.
const MAX_FONT_TASKS: usize = 4;

/// Lifecycle, presentation and enrollment requests owned by one workspace session.
#[derive(Debug)]
pub(crate) enum SessionCommand {
    /// Request an enrollment workflow on the owning workspace session.
    Enrollment(crate::enrollment::EnrollmentCommand),
    /// Resolve presentation fonts on session-owned workers, without touching account data.
    Font(crate::ui::fonts::FontOperation),
    /// Retry only after the previous failed open has settled.
    RetryOpen,
    /// Stop admitting work and begin resource cleanup.
    BeginShutdown,
}

/// Immutable non-secret state for native presentation.
#[derive(Clone, Debug)]
pub(crate) enum SessionStatus {
    /// Database initialization and initial reads are in progress.
    Opening,
    /// The real store and initial persisted defaults are available.
    Ready {
        /// Stable local account and enrollment status.
        identity: WorkspaceStatus,
        /// Tag whose known deletion caused startup to select All contacts.
        restored_view_from: Option<clacks_model::ids::TagId>,
    },
    /// Opening failed without replacing existing files.
    Failed {
        /// Actionable local diagnostic; never emitted wholesale to telemetry.
        message: String,
    },
    /// Cleanup remains active; the window must remain responsive.
    Closing {
        /// Current cleanup resource family.
        phase: &'static str,
        /// Original monotonic close request time, unchanged by repeated requests.
        started: Instant,
    },
    /// No workspace, activation or telemetry work remains.
    Closed,
}

/// Resources transferred to the runtime while outer composition retains ownership.
pub(crate) struct SessionResources {
    /// Nonsecret enrollment progress delivered to the native connection dialog.
    pub enrollment_updates: watch::Sender<crate::enrollment::EnrollmentStatus>,
    /// Selected physical workspace and output locations.
    pub paths: WorkspacePaths,
    /// Bound endpoint, already accepting before database initialization.
    pub listener: ActivationListener,
    /// Providers and writer retained through work draining.
    pub telemetry: TelemetryGuard,
    /// Activation notices, independent of the UI's own receiver.
    pub activation: watch::Receiver<()>,
    /// Attached native context, absent while the renderer starts.
    pub context: Arc<OnceLock<egui::Context>>,
    /// Bounded requests from actual settings editors.
    pub settings_requests: mpsc::Receiver<settings::SettingsRequest>,
    /// Ordered committed settings view for native presentation.
    pub settings_updates: watch::Sender<Option<Arc<SettingsSnapshot>>>,
}

/// Publish state and wake the native loop without relying on redraws for liveness.
fn publish(
    status: &watch::Sender<SessionStatus>,
    context: &OnceLock<egui::Context>,
    next: SessionStatus,
) {
    status.send_replace(next);
    if let Some(context) = context.get() {
        context.request_repaint();
    }
}

/// Own workspace work until it settles, then close every resource before acknowledging exit.
pub(crate) async fn run(
    resources: SessionResources,
    mut commands: mpsc::Receiver<SessionCommand>,
    status: watch::Sender<SessionStatus>,
) -> anyhow::Result<()> {
    let SessionResources {
        enrollment_updates,
        paths,
        listener,
        telemetry,
        activation,
        context,
        mut settings_requests,
        settings_updates,
    } = resources;

    let stop = CancellationToken::new();
    let notice_task = tokio::spawn(activation_notices(
        activation,
        context.clone(),
        stop.clone(),
    ));

    let mut work = JoinSet::new();
    let mut font_work = JoinSet::new();
    let mut enrollment_work = JoinSet::new();
    let mut enrollment_progress = enrollment_updates.subscribe();
    let mut enrollment_stop = CancellationToken::new();
    let mut settings_open = true;
    let mut opening = JoinSet::new();
    opening::begin(&mut opening, paths.clone());

    let mut attempt_started = Instant::now();
    let mut opened: Option<OpenedWorkspace> = None;
    let mut closing = None;
    let mut commands_open = true;

    let mut loss_count = 0;
    let file_failure = telemetry.file_failure();
    let mut file_failure_reported = false;
    let mut loss_check = tokio::time::interval(Duration::from_secs(5));

    loop {
        if closing.is_some()
            && opening.is_empty()
            && work.is_empty()
            && font_work.is_empty()
            && enrollment_work.is_empty()
        {
            break;
        }

        tokio::select! {
            biased;
            command = commands.recv(), if commands_open => match command {
                Some(SessionCommand::Enrollment(EnrollmentCommand::Cancel)) => {
                    enrollment_stop.cancel();
                },
                Some(SessionCommand::Enrollment(command)) if closing.is_none() && enrollment_work.is_empty() => {
                    if let Some(workspace) = &opened {
                        enrollment_stop = CancellationToken::new();
                        enrollment_work.spawn(
                            worker::execute(
                                workspace.store.clone(),
                                paths.clone(),
                                command,
                                enrollment_updates.clone(),
                                enrollment_stop.clone(),
                            )
                            .in_current_span()
                            .with_current_subscriber(),
                        );
                    }
                }
                Some(SessionCommand::Enrollment(_)) => {},
                Some(SessionCommand::Font(operation)) if closing.is_none() && font_work.len() < MAX_FONT_TASKS => {
                    let span = tracing::Span::current();
                    let dispatcher = tracing::dispatcher::get_default(Clone::clone);
                    font_work.spawn_blocking(move || tracing::dispatcher::with_default(&dispatcher, || span.in_scope(|| operation.run())));
                }
                Some(SessionCommand::Font(operation)) => {
                    operation.reject(if closing.is_some() { "The application is closing." } else { "Font loading is busy. Retry after current loads finish." });
                },
                Some(SessionCommand::RetryOpen) if closing.is_none() && opening.is_empty() && opened.is_none() => {
                    publish(&status, &context, SessionStatus::Opening);
                    attempt_started = Instant::now();
                    opening::begin(&mut opening, paths.clone());
                    tracing::info!(event = "desktop.workspace_retry");
                }
                Some(SessionCommand::RetryOpen) => {},
                Some(SessionCommand::BeginShutdown) | None => {
                    if closing.is_none() {
                        let started = Instant::now();
                        closing = Some(started);
                        enrollment_stop.cancel();
                        settings_requests.close();
                        while let Ok(request) = settings_requests.try_recv() { let _ = request.reply.send(settings::SettingsResponse::Closing); }
                        tracing::info!(event = "desktop.shutdown_requested");
                        publish(
                            &status,
                            &context,
                            SessionStatus::Closing {
                                phase: "Finishing workspace work",
                                started,
                            },
                        );
                    }
                    // A disconnected UI must not leave a permanently-ready recv branch.
                    if commands.is_closed() {
                        commands_open = false;
                    }
                }
            },
            result = font_work.join_next(), if !font_work.is_empty() => {
                if matches!(result, Some(Err(_))) { tracing::error!(event = "desktop.font_task_failed"); }
                if let Some(context) = context.get() { context.request_repaint(); }
            },
            result = enrollment_work.join_next(), if !enrollment_work.is_empty() => {
                if let Some(result) = result {
                    let outcome = match result {
                        Ok(outcome) => outcome,
                        Err(_) => EnrollmentStatus::Failed {
                            message: "The enrollment worker stopped unexpectedly; retry the saved attempt.".into(),
                            resume_available: true,
                            discard_available: false,
                            destination: None,
                            destination_change_available: false,
                        },
                    };
                    enrollment_updates.send_replace(outcome);
                }
            },
            _ = enrollment_progress.changed() => {
                enrollment_progress.borrow_and_update();
                if let Some(context) = context.get() {context.request_repaint();}
            },
            result = opening.join_next(), if !opening.is_empty() => {
                if let Some(result) = result {
                    let result = result.map_err(anyhow::Error::from).and_then(std::convert::identity);
                    let outcome = if closing.is_some() {
                        "cancelled"
                    } else if result.is_ok() {
                        "ready"
                    } else {
                        "failed"
                    };
                    metrics::histogram!(STARTUP_DURATION_METRIC, "outcome" => outcome)
                        .record(attempt_started.elapsed().as_secs_f64());

                    match result {
                        Ok(workspace) => {
                            if closing.is_none() {
                                let enrollment = match worker::inspect(&workspace.store).await {
                                    Ok(enrollment) => enrollment,
                                    Err(error) => EnrollmentStatus::Failed {
                                        message: error.to_string(),
                                        resume_available: false,
                                        discard_available: false,
                                        destination: None,
                                        destination_change_available: false,
                                    },
                                };
                                enrollment_updates.send_replace(enrollment);
                                settings_updates.send_replace(Some(Arc::new(workspace.settings.clone())));
                                tracing::info!(
                                    event = "desktop.workspace_ready",
                                    account_id = %workspace.identity.account,
                                    enrolled = workspace.identity.enrolled,
                                );
                                publish(
                                    &status,
                                    &context,
                                    SessionStatus::Ready {
                                        identity: workspace.identity,
                                        restored_view_from: workspace.restored_view_from,
                                    },
                                );
                            }
                            opened = Some(workspace);
                        }
                        Err(error) => {
                            tracing::error!(event = "desktop.workspace_open_failed");
                            if closing.is_none() {
                                publish(
                                    &status,
                                    &context,
                                    SessionStatus::Failed {
                                        message: format!("Could not open the workspace: {error:#}. Correct the problem and retry, or select another directory with --workspace."),
                                    },
                                );
                            }
                        }
                    }
                }
            },
            request = settings_requests.recv(), if settings_open && closing.is_none() && work.is_empty() => {
                match request {
                    Some(request) => {
                        if let Some(workspace) = &opened {
                            work.spawn(settings::execute(workspace.store.clone(), request).in_current_span().with_current_subscriber());
                        } else {
                            let _ = request.reply.send(settings::SettingsResponse::Failed("The local workspace is not ready".into()));
                        }
                    }
                    None => settings_open = false,
                }
            },
            completion = work.join_next(), if !work.is_empty() => {
                match completion {
                    Some(Ok(Some(completion))) => {
                        if let Some(snapshot) = completion.snapshot {
                            if let Some(previous) = settings_updates.borrow().as_ref() {
                                if let Some(profile) = &previous.operating.selected_profile
                                    && snapshot.operating.selected_profile.is_none()
                                    && let Some(deleted) = snapshot.profiles.iter().find(|row| row.key.id == clacks_sync::record::RecordId::Profile(profile.id()) && matches!(row.working_state(), Some(clacks_sync::record::RecordState::Deleted))) {
                                    tracing::info!(event = "operating.deleted_profile_forgotten", account_id = %deleted.key.account, profile_id = %profile.id());
                                }
                                if let Some(equipment) = &previous.operating.selected_equipment
                                    && snapshot.operating.selected_equipment.is_none()
                                    && let Some(deleted) = snapshot.equipment.iter().find(|row| row.key.id == clacks_sync::record::RecordId::Equipment(equipment.id()) && matches!(row.working_state(), Some(clacks_sync::record::RecordState::Deleted))) {
                                    tracing::info!(event = "operating.deleted_equipment_forgotten", account_id = %deleted.key.account, equipment_id = %equipment.id());
                                }
                            }
                            settings_updates.send_replace(Some(Arc::new(snapshot)));
                        }
                        let _ = completion.reply.send(completion.response);
                        if let Some(context) = context.get() { context.request_repaint(); }
                    }
                    Some(Ok(None)) => {},
                    Some(Err(_)) => tracing::error!(event = "desktop.settings_task_failed"),
                    None => {},
                }
            },
            _ = loss_check.tick() => {
                if file_failure.load(std::sync::atomic::Ordering::Relaxed) && !file_failure_reported {
                    file_failure_reported = true;
                    tracing::warn!(event = "desktop.local_log_write_failed");
                    if let Some(context) = context.get() {
                        context.request_repaint();
                    }
                }

                let dropped = telemetry.dropped_lines();
                if dropped != loss_count {
                    tracing::warn!(event = "desktop.local_log_loss", dropped_lines = dropped);
                    loss_count = dropped;
                    if let Some(context) = context.get() {
                        context.request_repaint();
                    }
                }
            }
        }
    }

    let started = closing.unwrap_or_else(Instant::now);
    shutdown::finish(
        shutdown::ShutdownResources {
            opened,
            listener,
            telemetry,
            stop,
            notices: notice_task,
        },
        &status,
        &context,
        started,
    )
    .await
}

/// Wake hidden windows on activation while leaving early notices available to the UI.
async fn activation_notices(
    mut activation: watch::Receiver<()>,
    context: Arc<OnceLock<egui::Context>>,
    stop: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = stop.cancelled() => break,
            result = activation.changed() => {
                if result.is_err() {
                    break;
                }
                tracing::info!(event = "desktop.activation_received");
                if let Some(context) = context.get() {
                    context.request_repaint();
                }
            }
        }
    }
}
