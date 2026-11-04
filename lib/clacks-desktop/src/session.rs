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
    /// Configure the workspace-owned WS Suite receiver.
    WsSuite(crate::ws_suite::ReceiverRequest),
    /// Request an enrollment workflow on the owning workspace session.
    Enrollment(crate::enrollment::EnrollmentCommand),
    /// Resolve presentation fonts on session-owned workers, without touching account data.
    Font(crate::ui::fonts::FontOperation),
    /// Retry only after the previous failed open has settled.
    RetryOpen,
    /// Retry local directory access without restarting synchronization or authorization.
    RetryDirectoryStorage,
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
        /// Current native connection or error, independently of pending record resolution.
        synchronization: crate::synchronization::Status,
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
    /// Receiver preference/failure output owned by this workspace.
    pub ws_suite: crate::ws_suite::Outputs,
    /// Shared directory client for the active workspace and its consumers.
    pub lookup: watch::Sender<Option<crate::directory::lookup::LookupClient>>,
    /// Local commit notification shared with both sync and native history.
    pub contact_changes: watch::Sender<()>,
    /// Directory installation and diagnostic outputs owned by this workspace session.
    pub directory: crate::directory::provisioning::Outputs,
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

/// Deliver settled settings work before enrollment or normal session admission continues.
fn settle_settings(
    result: Result<Option<settings::Completion>, tokio::task::JoinError>,
    settings_updates: &watch::Sender<Option<Arc<SettingsSnapshot>>>,
    context: &OnceLock<egui::Context>,
) {
    match result {
        Ok(Some(completion)) => {
            if let Some(snapshot) = completion.snapshot {
                if let Some(previous) = settings_updates.borrow().as_ref() {
                    if let Some(profile) = &previous.operating.selected_profile()
                        && snapshot.operating.selected_profile().is_none()
                        && let Some(deleted) = snapshot.profiles.iter().find(|row| {
                            row.key().id == clacks_sync::record::RecordId::Profile(profile.id())
                                && matches!(
                                    row.working_state(),
                                    Some(clacks_sync::record::RecordState::Deleted)
                                )
                        })
                    {
                        tracing::info!(event = "operating.deleted_profile_forgotten", account_id = %deleted.key().account, profile_id = %profile.id());
                    }
                    if let Some(equipment) = &previous.operating.selected_equipment()
                        && snapshot.operating.selected_equipment().is_none()
                        && let Some(deleted) = snapshot.equipment.iter().find(|row| {
                            row.key().id == clacks_sync::record::RecordId::Equipment(equipment.id())
                                && matches!(
                                    row.working_state(),
                                    Some(clacks_sync::record::RecordState::Deleted)
                                )
                        })
                    {
                        tracing::info!(event = "operating.deleted_equipment_forgotten", account_id = %deleted.key().account, equipment_id = %equipment.id());
                    }
                }
                settings_updates.send_replace(Some(Arc::new(snapshot)));
            }
            let _ = completion.reply.send(completion.response);
            if let Some(context) = context.get() {
                context.request_repaint();
            }
        }
        Ok(None) => {}
        Err(_) => tracing::error!(event = "desktop.settings_task_failed"),
    }
}

/// Own workspace work until it settles, then close every resource before acknowledging exit.
pub(crate) async fn run(
    resources: SessionResources,
    mut commands: mpsc::Receiver<SessionCommand>,
    status: watch::Sender<SessionStatus>,
) -> anyhow::Result<()> {
    let SessionResources {
        ws_suite,
        lookup,
        contact_changes: local_changes,
        directory,
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
    let mut sync_work = JoinSet::new();
    let mut lookup_work = JoinSet::new();
    let mut receiver_work = JoinSet::new();
    let mut receiver_stop = CancellationToken::new();
    let mut receiver_pairs = crate::ws_suite::pairing::Pairing::default();
    let mut receiver_commands: Option<mpsc::Sender<crate::ws_suite::ReceiverRequest>> = None;
    let mut lookup_stop = CancellationToken::new();
    let (lookup_binding, lookup_binding_view) = watch::channel(None);
    let mut lookup_client: Option<crate::directory::lookup::LookupClient> = None;
    let mut sync_stop = CancellationToken::new();
    let (sync_updates, mut sync_progress) =
        watch::channel(crate::synchronization::Status::Disconnected);
    let mut directory_progress = directory.status.subscribe();
    let mut settings_progress = settings_updates.subscribe();
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
            && sync_work.is_empty()
            && receiver_work.is_empty()
            && lookup_work.is_empty()
        {
            break;
        }

        tokio::select! {
            biased;
            command = commands.recv(), if commands_open => match command {
                Some(SessionCommand::WsSuite(request)) => {
                    if closing.is_none()
                        && enrollment_work.is_empty()
                        && let Some(sender) = &receiver_commands
                    {
                        if let Err(error) = sender.try_send(request) {
                            let _ = error.into_inner().reply.send(
                                crate::ws_suite::ReceiverResponse::Failed(
                                    "The receiver is busy. Try again.".into(),
                                ),
                            );
                        }
                    } else {
                        let _ = request.reply.send(crate::ws_suite::ReceiverResponse::Closing);
                    }
                },
                Some(SessionCommand::Enrollment(EnrollmentCommand::Cancel)) => {
                    enrollment_stop.cancel();
                },
                Some(SessionCommand::Enrollment(command)) if closing.is_none() && enrollment_work.is_empty() => {
                    while let Some(result) = work.join_next().await {
                        settle_settings(result, &settings_updates, &context);
                    }
                    if let Some(workspace) = &opened {
                        receiver_stop.cancel();
                        while let Some(result) = receiver_work.join_next().await {
                            if let Ok(Ok(pairs)) = result {
                                receiver_pairs = pairs;
                            } else {
                                tracing::error!(event = "ws_suite.receiver_task_failed");
                            }
                        }
                        receiver_commands = None;
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
                Some(SessionCommand::RetryDirectoryStorage) => {
                    if closing.is_none() { directory.retry.notify_one(); }
                },
                Some(SessionCommand::BeginShutdown) | None => {
                    if closing.is_none() {
                        let started = Instant::now();
                        closing = Some(started);
                        enrollment_stop.cancel();
                        sync_stop.cancel();
                        lookup.send_replace(None); lookup_client = None; lookup_stop.cancel();
                        receiver_stop.cancel();
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
            result = receiver_work.join_next(), if !receiver_work.is_empty() => {
                if matches!(result, Some(Err(_)) | Some(Ok(Err(_)))) {
                    let mut view = ws_suite.updates.borrow().clone();
                    view.reception = crate::ws_suite::ReceptionState::Failed(
                        "WS Suite reception stopped unexpectedly. Reopen the workspace to retry.".into(),
                    );
                    ws_suite.publish(view);
                    tracing::error!(event = "ws_suite.receiver_task_failed");
                }
            },
            _ = lookup_work.join_next(), if !lookup_work.is_empty() => {},
            result = font_work.join_next(), if !font_work.is_empty() => {
                if matches!(result, Some(Err(_))) { tracing::error!(event = "desktop.font_task_failed"); }
                if let Some(context) = context.get() { context.request_repaint(); }
            },
            result = enrollment_work.join_next(), if !enrollment_work.is_empty() => {
                if let Some(result) = result {
                    let outcome = match result {
                        Ok(outcome) => outcome,
                        Err(_) => EnrollmentStatus::Failed {
                            enrolled: false,
                            message: "The enrollment worker stopped unexpectedly; try connecting again.".into(),
                            details: None,
                        },
                    };
                    enrollment_updates.send_replace(outcome);
                }
            },
            _ = enrollment_progress.changed() => {
                let ready = matches!(*enrollment_progress.borrow_and_update(), EnrollmentStatus::Ready);
                let binding_changed = if let Some(workspace) = &opened {
                    let installed = clacks_client_store::identity::binding(&workspace.store).await?;
                    installed != *lookup_binding.borrow()
                } else { false };
                if enrollment_work.is_empty() && (ready || binding_changed) && closing.is_none() && let Some(workspace) = opened.take() {
                    sync_stop.cancel();
                    while let Some(result) = sync_work.join_next().await { if let Err(error) = result { tracing::error!(event="sync.worker.failed", error.type=if error.is_panic() { "task_panic" } else { "task_cancelled" }); } }
                    lookup_stop.cancel();
                    while lookup_work.join_next().await.is_some() {}
                    lookup_client = None; lookup.send_replace(None);
                    receiver_stop.cancel();
                    while receiver_work.join_next().await.is_some() {}
                    receiver_commands = None;
                    receiver_stop = CancellationToken::new();
                    receiver_pairs = Default::default();
                    workspace.store.close().await?;
                    sync_stop = CancellationToken::new();
                    lookup_stop = CancellationToken::new();
                    attempt_started = Instant::now();
                    publish(&status, &context, SessionStatus::Opening);
                    opening::begin(&mut opening, paths.clone());
                }
                if enrollment_work.is_empty()
                    && closing.is_none()
                    && receiver_commands.is_none()
                    && let Some(lookup) = &lookup_client
                    && let Some(workspace) = &opened
                {
                    receiver_stop = CancellationToken::new();
                    let (sender, input) = mpsc::channel(8);
                    receiver_commands = Some(sender);
                    receiver_work.spawn(
                        crate::ws_suite::runtime::run(
                            crate::ws_suite::runtime::Resources {
                                store: workspace.store.clone(), lookup:lookup.clone(),
                                settings_updates:settings_updates.clone(), changes:local_changes.clone(),
                            },
                            input,
                            ws_suite.clone(),
                            std::mem::take(&mut receiver_pairs),
                            receiver_stop.clone(),
                        ).in_current_span().with_current_subscriber(),
                    );
                }
                if let Some(context) = context.get() {context.request_repaint();}
            },
            _ = settings_progress.changed() => {
                settings_progress.borrow_and_update();
                if let Some(context) = context.get() {context.request_repaint();}
            },
            _ = sync_progress.changed() => {
                let synchronization = sync_progress.borrow_and_update().clone();
                let next = match &*status.borrow() {
                    SessionStatus::Ready {identity, restored_view_from, ..} if closing.is_none() => Some(SessionStatus::Ready {identity:*identity, restored_view_from:*restored_view_from, synchronization}),
                    _ => None,
                };
                if let Some(next) = next {publish(&status, &context, next);}
            },
                        result = sync_work.join_next(), if !sync_work.is_empty() => {
                if let Some(Err(error)) = result {
                    tracing::error!(event="sync.worker.failed", error.type=if error.is_panic() { "task_panic" } else { "task_cancelled" });
                    sync_updates.send_replace(crate::synchronization::Status::Failed("Synchronization task stopped unexpectedly".into()));
                }
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
                                    Err(_error) => EnrollmentStatus::Failed {
                                        enrolled: workspace.identity.enrolled,
                                        message: "Could not read connection settings. Try connecting again.".into(),
                                        details: None,
                                    },
                                };
                                // A watch channel coalesces updates. Preserve the terminal
                                // result through reopening so the dialog cannot miss success
                                // or an actionable post-install cleanup failure.
                                if !matches!(*enrollment_updates.borrow(), EnrollmentStatus::Ready | EnrollmentStatus::Failed { .. }) {
                                    enrollment_updates.send_replace(enrollment);
                                }
                                settings_updates.send_replace(Some(Arc::new(workspace.settings.clone())));
                                lookup_binding.send_replace(clacks_client_store::identity::binding(&workspace.store).await?);
                                let (client, runtime) = crate::directory::lookup::runtime::LookupRuntime::new(workspace.store.clone(), lookup_binding_view.clone(), directory.installed.subscribe(), context.clone()).await;
                                lookup.send_replace(Some(client.clone()));
                                lookup_client = Some(client.clone());
                                lookup_work.spawn(runtime.run(lookup_stop.clone()));
                                tracing::info!(
                                    event = "desktop.workspace_ready",
                                    account_id = %workspace.identity.account,
                                    enrolled = workspace.identity.enrolled,
                                );
                                publish(
                                    &status,
                                    &context,
                                    SessionStatus::Ready {
                                        synchronization: crate::synchronization::Status::Disconnected,
                                        identity: workspace.identity,
                                        restored_view_from: workspace.restored_view_from,
                                    },
                                );
                                let (receiver_sender, receiver_input) = mpsc::channel(8);
                                receiver_commands = Some(receiver_sender);
                                receiver_work.spawn(
                                    crate::ws_suite::runtime::run(
                                        crate::ws_suite::runtime::Resources {
                                            store:workspace.store.clone(), lookup:client.clone(),
                                            settings_updates:settings_updates.clone(), changes:local_changes.clone(),
                                        },
                                        receiver_input,
                                        ws_suite.clone(),
                                        std::mem::take(&mut receiver_pairs),
                                        receiver_stop.clone(),
                                    ).in_current_span().with_current_subscriber(),
                                );
                                start_sync(&mut sync_work, &workspace.store, &local_changes, &sync_updates, &settings_updates, &sync_stop, &directory).await?;
                            }
                            opened = Some(workspace);
                        }
                        Err(error) => {
                            tracing::error!(event = "desktop.workspace_open_failed", error.type = crate::telemetry::failure::error_class(&error));
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
            request = settings_requests.recv(), if settings_open && closing.is_none() && work.is_empty() && enrollment_work.is_empty() => {
                match request {
                    Some(request) => {
                        if let Some(workspace) = &opened {
                            let lookup = lookup_client.clone();
                            let store = workspace.store.clone();
                            let changes = local_changes.clone();
                            work.spawn(async move {
                                let shared = request.action.is_shared_write();
                                let result = settings::execute(store, request, lookup, changes.clone()).await;
                                if shared && result.is_some() {changes.send_replace(());}
                                result
                            }.in_current_span().with_current_subscriber());
                        } else {
                            let _ = request.reply.send(settings::SettingsResponse::Failed("The local workspace is not ready".into()));
                        }
                    }
                    None => settings_open = false,
                }
            },
            completion = work.join_next(), if !work.is_empty() => {
                if let Some(completion) = completion { settle_settings(completion, &settings_updates, &context); }
            },
            changed = directory_progress.changed() => {
                if changed.is_ok() && let Some(context) = context.get() { context.request_repaint(); }
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

/// Start the actual enrolled workspace's driver under the owning session lifetime.
async fn start_sync(
    tasks: &mut JoinSet<()>,
    store: &clacks_client_store::store::ClientStore,
    changes: &watch::Sender<()>,
    status: &watch::Sender<crate::synchronization::Status>,
    updates: &watch::Sender<Option<Arc<SettingsSnapshot>>>,
    stop: &CancellationToken,
    directory: &crate::directory::provisioning::Outputs,
) -> anyhow::Result<()> {
    if let Some(binding) = clacks_client_store::identity::binding(store).await? {
        tasks.spawn(
            crate::synchronization::run(
                store.clone(),
                binding,
                changes.subscribe(),
                status.clone(),
                updates.clone(),
                stop.clone(),
                directory.clone(),
            )
            .in_current_span()
            .with_current_subscriber(),
        );
    }
    Ok(())
}
