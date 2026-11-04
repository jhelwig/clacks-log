//! Start the desktop runtime, workspace session, telemetry and native window.

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

/// Histogram bucket limits, in seconds, for workspace startup duration.
pub(crate) const STARTUP_DURATION_BOUNDS: &[f64] =
    &[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0];

/// Compose ownership and runtime before entering the main-thread native event loop.
pub(crate) fn run(config: DesktopConfig) -> anyhow::Result<()> {
    // Workspace feature unification can enable both Rustls providers. Select
    // AWS-LC before telemetry or background tasks build TLS clients; an existing
    // process provider is retained if one has already been installed.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

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
    let (enrollment_updates, enrollment_view) =
        watch::channel(crate::enrollment::EnrollmentStatus::default());
    let directory = crate::directory::provisioning::Outputs {
        installed: watch::channel(None).0,
        status: watch::channel(Default::default()).0,
        retry: Default::default(),
    };
    let directory_status = directory.status.subscribe();
    let (lookup_updates, lookup_view) = mpsc::channel(32);
    let (ws_suite_updates, ws_suite_view) = watch::channel(Default::default());
    let resources = SessionResources {
        ws_suite: crate::ws_suite::Outputs {
            updates: ws_suite_updates,
            context: context.clone(),
        },
        lookup: crate::directory::lookup::Outputs {
            updates: lookup_updates,
            context: context.clone(),
        },
        directory,
        enrollment_updates,
        settings_requests: requests,
        settings_updates,
        paths: paths.clone(),
        listener,
        telemetry,
        activation: activation.clone(),
        context: context.clone(),
    };
    let (commands, input) = mpsc::channel(8);
    let (updates, status) = watch::channel(SessionStatus::Opening);
    let task = runtime.spawn(session::run(resources, input, updates));

    let ui_commands = commands.clone();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([680.0, 440.0]),
        persistence_path: Some(paths.directory.join("desktop-ui")),
        ..Default::default()
    };

    let window = run_window(
        options,
        context,
        UiResources {
            ws_suite_view,
            lookup_view,
            directory_status,
            enrollment_view,
            status,
            activation,
            commands: ui_commands,
            paths,
            diagnostics,
            loss,
            file_failure,
            settings_requests,
            settings_view,
        },
    );
    if let Err(error) = &window {
        tracing::error!(%error, "Desktop initialization failed");
    }

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

/// Run native initialization inside an error boundary so session cleanup always runs.
fn run_window(
    options: eframe::NativeOptions,
    context: Arc<OnceLock<eframe::egui::Context>>,
    resources: UiResources,
) -> anyhow::Result<()> {
    #[cfg(any(target_os = "macos", windows))]
    let mut menus = crate::menus::native::MenuAdapter::prepare()?;
    #[cfg(any(target_os = "macos", windows))]
    let options = {
        let mut options = options;
        menus.configure(&mut options);
        options
    };
    eframe::run_native(
        "Clacks Log",
        options,
        Box::new(move |creation| {
            let _ = context.set(creation.egui_ctx.clone());
            let mut app = DesktopApp::new(resources);
            app.restore_appearance(&creation.egui_ctx, creation.storage);
            #[cfg(any(target_os = "macos", windows))]
            {
                let (sender, receiver) = std::sync::mpsc::channel();
                menus.attach(creation, sender)?;
                app.install_menus(menus, receiver);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| anyhow::anyhow!("native window failed: {error}"))
}
