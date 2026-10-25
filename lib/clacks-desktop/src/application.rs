//! Desktop application composition and its owner-startup measurement definition.

use crate::{
    config::DesktopConfig,
    instance::{self, Election},
    session::{self, SessionCommand, SessionResources, SessionStatus},
    telemetry,
    ui::{DesktopApp, UiResources},
    workspace::WorkspacePaths,
};
use std::sync::{Arc, OnceLock};
use tokio::sync::{mpsc, watch};

/// Seconds elapsed for one owning-process workspace startup attempt.
pub(crate) const STARTUP_DURATION_METRIC: &str = "clacks_desktop_startup_duration_seconds";

/// Finite second boundaries spanning ordinary startup and slow migration work.
pub(crate) const STARTUP_DURATION_BOUNDS: &[f64] =
    &[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0];

/// Compose ownership and runtime before entering the main-thread native event loop.
pub(crate) fn run(config: DesktopConfig) -> anyhow::Result<()> {
    let paths = WorkspacePaths::resolve(&config)?;
    let owner = match instance::elect(&paths)? {
        Election::Owner(owner) => owner,
        Election::Contended(address) => match instance::activation::request(&address) {
            Ok(()) => return Ok(()),
            Err(error) => match instance::elect(&paths)? {
                Election::Owner(owner) => owner,
                Election::Contended(_) => return Err(error),
            },
        },
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("clacks-runtime")
        .build()?;
    let (notices, activation) = watch::channel(());
    let listener = owner.listen(runtime.handle(), notices)?;

    let telemetry = telemetry::initialize(&config, &paths);
    let diagnostics = telemetry.diagnostics().to_vec();
    let loss = telemetry.loss_counter();
    let file_failure = telemetry.file_failure();

    metrics::describe_histogram!(
        STARTUP_DURATION_METRIC,
        metrics::Unit::Seconds,
        "Elapsed owner workspace startup attempt, by ready/failed/cancelled outcome"
    );
    tracing::info!(event = "desktop.starting");

    let context = Arc::new(OnceLock::new());
    let (settings_requests, requests) = mpsc::channel(8);
    let (settings_updates, settings_view) = watch::channel(None);
    let resources = SessionResources {
        settings_requests: requests,
        settings_updates,
        paths: paths.clone(),
        listener,
        telemetry,
        activation: activation.clone(),
        context: context.clone(),
        #[cfg(test)]
        opening_gate: None,
    };
    let (commands, input) = mpsc::channel(8);
    let (updates, status) = watch::channel(SessionStatus::Opening);
    let task = runtime.spawn(session::run(resources, input, updates));

    let ui_commands = commands.clone();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([680.0, 440.0]),
        ..Default::default()
    };

    let window = eframe::run_native(
        "Clacks Log",
        options,
        Box::new(move |creation| {
            let _ = context.set(creation.egui_ctx.clone());
            Ok(Box::new(DesktopApp::new(UiResources {
                status,
                activation,
                commands: ui_commands,
                paths,
                diagnostics,
                loss,
                file_failure,
                settings_requests,
                settings_view,
            })))
        }),
    );

    // This fallback also runs when the renderer fails before constructing the app.
    let cleanup = runtime.block_on(async {
        let _ = commands.send(SessionCommand::BeginShutdown).await;
        task.await?
    });
    drop(runtime);
    drop(owner);

    if let Err(error) = window {
        return Err(anyhow::anyhow!("native window failed: {error}"));
    }

    cleanup
}
