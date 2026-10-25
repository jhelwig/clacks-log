//! Private view-state tests without a display server.

use super::*;
use std::time::Instant;

struct Window {
    app: DesktopApp,
    context: egui::Context,
    updates: watch::Sender<SessionStatus>,
    notices: watch::Sender<()>,
    commands: mpsc::Receiver<SessionCommand>,
    _writer: tracing_appender::non_blocking::WorkerGuard,
}

impl Window {
    fn new() -> Self {
        let (updates, status) = watch::channel(SessionStatus::Opening);
        let (notices, activation) = watch::channel(());
        let (commands, input) = mpsc::channel(8);
        let (output, writer) = tracing_appender::non_blocking(std::io::sink());
        let paths = WorkspacePaths {
            directory: "workspace".into(),
            database: "workspace/workspace.sqlite".into(),
            lock_file: "workspace/instance.lock".into(),
            log_directory: "workspace/logs".into(),
        };
        let (settings_requests, _requests) = mpsc::channel(8);
        let (_settings_updates, settings_view) = watch::channel(None);
        Self {
            app: DesktopApp::new(UiResources {
                status,
                activation,
                commands,
                paths,
                diagnostics: Vec::new(),
                loss: output.error_counter(),
                file_failure: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                settings_requests,
                settings_view,
            }),
            context: Default::default(),
            updates,
            notices,
            commands: input,
            _writer: writer,
        }
    }

    fn update(&mut self) -> egui::LogicOutput {
        self.context.run_logic(&Default::default(), |context| {
            self.app.update_lifecycle(context)
        })
    }
}

fn has_command(output: &egui::LogicOutput, expected: &egui::ViewportCommand) -> bool {
    output
        .viewport_commands
        .values()
        .flatten()
        .any(|command| command == expected)
}

#[test]
fn activation_before_ready_is_retained() {
    let mut window = Window::new();
    window.notices.send_replace(());

    let output = window.update();

    assert!(has_command(&output, &egui::ViewportCommand::Visible(true)));
    assert!(has_command(
        &output,
        &egui::ViewportCommand::Minimized(false)
    ));

    assert!(has_command(&output, &egui::ViewportCommand::Focus));
    assert!(!has_command(
        &window.update(),
        &egui::ViewportCommand::Focus
    ));
}

#[test]
fn delayed_close_does_not_automatically_exit_and_repeated_requests_do_not_restart() {
    let mut window = Window::new();
    window.app.begin_close();
    window.app.begin_close();
    window.update();

    assert!(matches!(
        window.commands.try_recv(),
        Ok(SessionCommand::BeginShutdown)
    ));

    assert!(window.commands.try_recv().is_err());
    let started = Instant::now() - Duration::from_secs(6);
    window.updates.send_replace(SessionStatus::Closing {
        phase: "Finishing work",
        started,
    });

    let output = window.update();

    assert!(window.app.delayed_reported);
    assert!(!has_command(&output, &egui::ViewportCommand::Close));
    assert!(!closing::delayed(Instant::now()));
    assert!(closing::delayed(started));
    window.updates.send_replace(SessionStatus::Closed);

    assert!(has_command(&window.update(), &egui::ViewportCommand::Close));
}

#[test]
fn native_close_while_failed_is_cancelled_until_cleanup_acknowledges() {
    let mut window = Window::new();
    window.updates.send_replace(SessionStatus::Failed {
        message: "Invalid database".into(),
    });
    let mut input = egui::RawInput::default();
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);

    let output = window
        .context
        .run_logic(&input, |context| window.app.update_lifecycle(context));

    assert!(has_command(&output, &egui::ViewportCommand::CancelClose));
    assert!(matches!(
        window.commands.try_recv(),
        Ok(SessionCommand::BeginShutdown)
    ));
}
