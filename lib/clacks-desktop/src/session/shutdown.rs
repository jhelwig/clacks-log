//! Ordered cleanup with state updates while the native window remains responsive.

use super::{SessionStatus, publish};
use crate::{
    instance::activation::ActivationListener, telemetry::TelemetryGuard, workspace::OpenedWorkspace,
};
use eframe::egui;
use std::{sync::OnceLock, time::Instant};
use tokio::{sync::watch, task::JoinHandle};
use tokio_util::sync::CancellationToken;

/// Resources remaining after workspace admission has stopped and opening has settled.
pub(super) struct ShutdownResources {
    /// Any completed workspace, including one returned after a close request.
    pub opened: Option<OpenedWorkspace>,
    /// Bound activation listener and handler tasks.
    pub listener: ActivationListener,
    /// Standard diagnostic providers and writer guard.
    pub telemetry: TelemetryGuard,
    /// Stop signal for the activation repaint task.
    pub stop: CancellationToken,
    /// Tracked activation repaint task.
    pub notices: JoinHandle<()>,
}

/// Close completed workspace work, activation and diagnostic workers in ownership order.
pub(super) async fn finish(
    resources: ShutdownResources,
    status: &watch::Sender<SessionStatus>,
    context: &OnceLock<egui::Context>,
    started: Instant,
) -> anyhow::Result<()> {
    let ShutdownResources {
        opened,
        listener,
        telemetry,
        stop,
        notices,
    } = resources;
    let mut failures = Vec::new();
    publish(
        status,
        context,
        SessionStatus::Closing {
            phase: "Closing local storage",
            started,
        },
    );
    if let Some(opened) = opened
        && let Err(error) = opened.store.close().await
    {
        failures.push(error);
    }
    publish(
        status,
        context,
        SessionStatus::Closing {
            phase: "Stopping activation",
            started,
        },
    );
    if let Err(error) = listener.shutdown().await {
        failures.push(error);
    }
    stop.cancel();
    if let Err(error) = notices.await {
        failures.push(error.into());
    }
    tracing::info!(
        event = "desktop.work_drained",
        cleanup_failures = failures.len(),
        file_output_failed = telemetry
            .file_failure()
            .load(std::sync::atomic::Ordering::Relaxed),
        dropped_log_lines = telemetry.dropped_lines()
    );
    publish(
        status,
        context,
        SessionStatus::Closing {
            phase: "Flushing diagnostics",
            started,
        },
    );
    match tokio::task::spawn_blocking(move || telemetry.shutdown()).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => failures.push(error),
        Err(error) => failures.push(error.into()),
    }
    publish(status, context, SessionStatus::Closed);
    if let Some(error) = failures.into_iter().next() {
        return Err(error);
    }
    Ok(())
}
