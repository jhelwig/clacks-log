//! Workspace session lifetime, independent of native redraws.

mod opening;
mod shutdown;
#[cfg(test)]
mod tests;

use crate::{
    application::STARTUP_DURATION_METRIC,
    instance::activation::ActivationListener,
    telemetry::TelemetryGuard,
    workspace::{OpenedWorkspace, WorkspacePaths},
};
use clacks_client_store::identity::WorkspaceStatus;
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

/// Lifecycle requests; ordinary domain work does not pass through this channel.
#[derive(Debug)]
pub(crate) enum SessionCommand {
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
        /// Whether persisted operating defaults have been selected.
        has_operating_location: bool,
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
    /// Test-only pause at the opening boundary; no shipped delay control.
    #[cfg(test)]
    pub opening_gate: Option<tokio::sync::oneshot::Receiver<()>>,
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
        paths,
        listener,
        telemetry,
        activation,
        context,
        #[cfg(test)]
        opening_gate,
    } = resources;
    let stop = CancellationToken::new();
    let notice_task = tokio::spawn(activation_notices(
        activation,
        context.clone(),
        stop.clone(),
    ));
    let mut opening = JoinSet::new();
    opening::begin(
        &mut opening,
        paths.clone(),
        #[cfg(test)]
        opening_gate,
    );
    let mut attempt_started = Instant::now();
    let mut opened: Option<OpenedWorkspace> = None;
    let mut closing = None;
    let mut commands_open = true;
    let mut loss_count = 0;
    let file_failure = telemetry.file_failure();
    let mut file_failure_reported = false;
    let mut loss_check = tokio::time::interval(Duration::from_secs(5));
    loop {
        if closing.is_some() && opening.is_empty() {
            break;
        }
        tokio::select! {
            biased;
            command = commands.recv(), if commands_open => match command {
                Some(SessionCommand::RetryOpen) if closing.is_none() && opening.is_empty() && opened.is_none() => {
                    publish(&status, &context, SessionStatus::Opening);
                    attempt_started = Instant::now();
                    opening::begin(&mut opening, paths.clone(), #[cfg(test)] None);
                    tracing::info!(event = "desktop.workspace_retry");
                }
                Some(SessionCommand::RetryOpen) => {},
                Some(SessionCommand::BeginShutdown) | None => {
                    if closing.is_none() {
                        let started = Instant::now();
                        closing = Some(started);
                        tracing::info!(event = "desktop.shutdown_requested");
                        publish(&status, &context, SessionStatus::Closing { phase: "Finishing workspace work", started });
                    }
                    // A disconnected UI must not leave a permanently-ready recv branch.
                    if commands.is_closed() { commands_open = false; }
                }
            },
            result = opening.join_next(), if !opening.is_empty() => {
                if let Some(result) = result {
                    let result = result.map_err(anyhow::Error::from).and_then(std::convert::identity);
                    let outcome = if closing.is_some() { "cancelled" } else if result.is_ok() { "ready" } else { "failed" };
                    metrics::histogram!(STARTUP_DURATION_METRIC, "outcome" => outcome).record(attempt_started.elapsed().as_secs_f64());
                    match result {
                        Ok(workspace) => {
                            if closing.is_none() {
                                tracing::info!(event = "desktop.workspace_ready", account_id = %workspace.identity.account, enrolled = workspace.identity.enrolled);
                                publish(&status, &context, SessionStatus::Ready { identity: workspace.identity, has_operating_location: workspace.operating.current.is_some() });
                            }
                            opened = Some(workspace);
                        }
                        Err(error) => {
                            tracing::error!(event = "desktop.workspace_open_failed");
                            if closing.is_none() {
                                publish(&status, &context, SessionStatus::Failed { message: format!("Could not open the workspace: {error:#}. Correct the problem and retry, or select another directory with --workspace.") });
                            }
                        }
                    }
                }
            },
            _ = loss_check.tick() => {
                if file_failure.load(std::sync::atomic::Ordering::Relaxed) && !file_failure_reported {
                    file_failure_reported = true;
                    tracing::warn!(event = "desktop.local_log_write_failed");
                    if let Some(context) = context.get() { context.request_repaint(); }
                }
                let dropped = telemetry.dropped_lines();
                if dropped != loss_count {
                    tracing::warn!(event = "desktop.local_log_loss", dropped_lines = dropped);
                    loss_count = dropped;
                    if let Some(context) = context.get() { context.request_repaint(); }
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
                if result.is_err() { break; }
                tracing::info!(event = "desktop.activation_received");
                if let Some(context) = context.get() { context.request_repaint(); }
            }
        }
    }
}
