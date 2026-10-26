//! Private view-state tests without a display server.

mod appearance;
mod main_screen;

use super::*;
use std::time::Instant;

#[derive(Default)]
struct Storage(std::collections::BTreeMap<String, String>);

impl eframe::Storage for Storage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

#[test]
fn appearance_apply_survives_app_save_and_reconstruction() {
    use super::appearance::AppearanceAction;
    use super::preferences::{Appearance, FontChoice};
    use eframe::App;

    let mut window = Window::new();
    let mut storage = Storage::default();
    storage.0.insert("unrelated".into(), "keep me".into());

    window
        .app
        .appearance
        .open(egui::Id::unique("test.appearance.opener"));
    let pending = &mut window.app.appearance.editor.as_mut().unwrap().pending;
    pending.appearance = Appearance::Light;
    pending.zoom = 1.25;
    pending.radio_font = FontChoice::Installed {
        family: "Example family".into(),
    };
    window
        .app
        .appearance
        .finish(&window.context, AppearanceAction::Apply);
    window.app.save(&mut storage);

    let mut reopened = Window::new();
    reopened
        .app
        .restore_appearance(&reopened.context, Some(&storage));
    reopened.draw(egui::RawInput::default());

    assert!(storage.0.contains_key("clacks.appearance.v1"));
    assert_eq!(storage.0.get("unrelated").unwrap(), "keep me");
    assert_eq!(reopened.context.theme(), egui::Theme::Light);
    assert_eq!(reopened.context.zoom_factor(), 1.25);
    assert_eq!(
        reopened.app.appearance.committed.radio_font,
        FontChoice::Installed {
            family: "Example family".into()
        }
    );
    assert!(!reopened.app.persist_egui_memory());
}

#[test]
fn appearance_cancelled_preview_is_not_saved() {
    use super::appearance::AppearanceAction;
    use super::preferences::Appearance;
    use eframe::App;

    let mut window = Window::new();
    window
        .app
        .appearance
        .open(egui::Id::unique("test.appearance.opener"));
    let pending = &mut window.app.appearance.editor.as_mut().unwrap().pending;
    pending.appearance = Appearance::Dark;
    pending.zoom = 1.5;
    window
        .app
        .appearance
        .finish(&window.context, AppearanceAction::Cancel);
    window.draw(egui::RawInput::default());
    let mut storage = Storage::default();
    window.app.save(&mut storage);
    let mut reopened = Window::new();
    reopened
        .app
        .restore_appearance(&reopened.context, Some(&storage));
    reopened.draw(egui::RawInput::default());

    assert_eq!(
        reopened.app.appearance.committed.appearance,
        Appearance::System
    );
    assert_eq!(reopened.context.zoom_factor(), 1.0);
}

#[test]
fn appearance_invalid_saved_preferences_do_not_prevent_startup() {
    let mut storage = Storage::default();
    storage.0.insert(
        "clacks.appearance.v1".into(),
        "not valid appearance data".into(),
    );
    let mut window = Window::new();

    window
        .app
        .restore_appearance(&window.context, Some(&storage));
    window.draw(egui::RawInput::default());

    assert_eq!(window.context.zoom_factor(), 1.0);
    assert!(window.app.appearance.notice.is_some());
    assert!(window.app.appearance.editor.is_none());
}

#[test]
fn appearance_composes_with_native_display_scale() {
    use super::preferences::{Appearance, AppearancePreferences};

    let mut window = Window::new();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .native_pixels_per_point = Some(2.0);
    theme::install(
        &window.context,
        &AppearancePreferences {
            appearance: Appearance::Light,
            zoom: 1.25,
            ..Default::default()
        },
    );
    window.draw(input);

    assert_eq!(window.context.pixels_per_point(), 2.5);
    for appearance in [egui::Theme::Light, egui::Theme::Dark] {
        let style = window.context.style_of(appearance);
        let colors = theme::palette(appearance);
        assert_ne!(colors.error, colors.edited_edge);
        assert_ne!(colors.error, style.visuals.selection.bg_fill);
    }
    for zoom in [f32::NAN, f32::INFINITY, 0.0, 0.74, 2.01] {
        assert!(
            AppearancePreferences {
                zoom,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}

#[test]
fn font_selection_assigns_the_requested_radio_role() {
    use super::preferences::{AppearancePreferences, FontChoice};

    let context = egui::Context::default();
    theme::install(
        &context,
        &AppearancePreferences {
            radio_font: FontChoice::Installed {
                family: "Selected radio family".into(),
            },
            ..Default::default()
        },
    );

    assert_eq!(
        context
            .global_style()
            .text_styles
            .get(&egui::TextStyle::Monospace)
            .unwrap()
            .family,
        egui::FontFamily::Name("clacks.radio".into())
    );
}

struct Window {
    app: DesktopApp,
    context: egui::Context,
    updates: watch::Sender<SessionStatus>,
    notices: watch::Sender<()>,
    commands: mpsc::Receiver<SessionCommand>,
    _writer: tracing_appender::non_blocking::WorkerGuard,
}

impl Window {
    fn draw(&mut self, input: egui::RawInput) {
        use eframe::App;

        let mut frame = eframe::Frame::_new_kittest();
        self.context
            .run_ui(input, |ui| self.app.ui(ui, &mut frame))
            .drop_without_applying_deltas();
    }

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

#[test]
fn closing_cancels_appearance_preview_and_keeps_delayed_shutdown_controls_accessible() {
    use super::preferences::AppearancePreferences;
    use eframe::App;
    let mut window = Window::new();
    window
        .app
        .appearance
        .open(egui::Id::unique("test.appearance.opener"));
    window.app.appearance.editor.as_mut().unwrap().pending.zoom = 1.5;
    theme::install(
        &window.context,
        &window.app.appearance.editor.as_ref().unwrap().pending,
    );
    window.draw(egui::RawInput::default());
    window.app.begin_close();
    window
        .updates
        .send(SessionStatus::Closing {
            phase: "Finishing work",
            started: Instant::now() - Duration::from_secs(6),
        })
        .unwrap();
    window.draw(egui::RawInput::default());
    let mut native_frame = eframe::Frame::_new_kittest();
    let mut output = window.context.run_ui(egui::RawInput::default(), |ui| {
        window.app.ui(ui, &mut native_frame)
    });
    output.textures_delta.clear();
    assert!(window.app.appearance.editor.is_none());
    assert_eq!(
        window.context.zoom_factor(),
        AppearancePreferences::default().zoom
    );
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == "Continue Waiting" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .unwrap();
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        window.draw(egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        });
    }
    assert!(window.app.continuing);
}
