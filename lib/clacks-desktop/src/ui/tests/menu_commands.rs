//! Menu actions exercise the production application and its existing destinations.
use super::*;
use crate::menus::MenuCommand;

#[test]
fn edit_select_all_selects_focused_text_without_contact_requests() {
    use eframe::App;
    use egui_kittest::{Harness, kittest::Queryable};
    struct Host {
        window: Window,
        text: String,
    }
    let mut window = Window::new();
    ready(&mut window);
    let (sender, mut requests) = mpsc::channel(8);
    window.app.settings_requests = sender;
    let mut ui = Harness::builder().build_ui_state(
        |ui, host: &mut Host| {
            let label = ui.label("Entry callsign");
            ui.text_edit_singleline(&mut host.text)
                .labelled_by(label.id);
        },
        Host {
            window,
            text: "W7ORIGINAL".into(),
        },
    );
    ui.get_by_label("Entry callsign").click();
    ui.run();
    let context = ui.ctx.clone();
    ui.state_mut()
        .window
        .app
        .dispatch_menu(&context, MenuCommand::SelectAll);
    let mut input = std::mem::take(ui.input_mut());
    // The native hook receives already gathered input, unlike Harness's event queue.
    input.events.push(egui::Event::Text("W7NEW".into()));
    ui.state_mut()
        .window
        .app
        .raw_input_hook(&context, &mut input);
    *ui.input_mut() = input;
    ui.run();
    assert_eq!(ui.state().text, "W7NEW");
    assert!(requests.try_recv().is_err());
}

fn ready(window: &mut Window) {
    window.updates.send_replace(SessionStatus::Ready {
        identity: clacks_client_store::identity::WorkspaceStatus {
            account: Default::default(),
            enrolled: false,
        },
        restored_view_from: None,
        synchronization: crate::synchronization::Status::Disconnected,
    });
    let (_, view) = watch::channel(Some(Arc::new(fixtures::snapshot())));
    window.app.settings_view = view;
}

#[test]
fn edit_select_all_dispatches_once_for_focused_history() -> anyhow::Result<()> {
    use crate::session::settings::{
        SettingsAction, SettingsResponse,
        contacts::{ContactAction, ContactResponse},
    };
    use egui_kittest::kittest::Queryable;
    let mut window = Window::new();
    ready(&mut window);
    let (sender, mut requests) = mpsc::channel(16);
    window.app.settings_requests = sender;
    let mut ui = main_screen::mount(window);
    let restore = requests.try_recv()?;
    assert!(matches!(
        restore.action,
        SettingsAction::Contacts(ContactAction::LoadQueryState)
    ));
    restore
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::QueryState(
            Default::default(),
        )))
        .unwrap();
    ui.run_steps(2);
    let page = requests.try_recv()?;
    assert!(matches!(
        page.action,
        SettingsAction::Contacts(ContactAction::Page { .. })
    ));
    page.reply
        .send(SettingsResponse::Contacts(ContactResponse::Page(
            fixtures::contact_page(0, 1, 1),
        )))
        .unwrap();
    ui.run_steps(2);
    ui.get_by_label("K1T000").click();
    ui.run_steps(2);
    let context = ui.ctx.clone();
    ui.state_mut()
        .app
        .dispatch_menu(&context, MenuCommand::SelectAll);
    ui.state_mut()
        .app
        .dispatch_menu(&context, MenuCommand::SelectAll);
    let mut selections = 0;
    while let Ok(request) = requests.try_recv() {
        match request.action {
            SettingsAction::Contacts(ContactAction::SelectAll { .. }) => selections += 1,
            SettingsAction::Contacts(ContactAction::Page { .. }) => {}
            _ => panic!("unexpected action"),
        }
    }
    assert_eq!(selections, 1);
    Ok(())
}

#[test]
fn menu_commands_respect_workspace_state() {
    let mut window = Window::new();
    assert!(
        !window
            .app
            .menu_enabled(&window.context, MenuCommand::ManageLogbooks)
    );
    assert!(
        window
            .app
            .menu_enabled(&window.context, MenuCommand::Appearance)
    );
    assert!(
        window
            .app
            .menu_enabled(&window.context, MenuCommand::Diagnostics)
    );
    ready(&mut window);
    assert!(
        window
            .app
            .menu_enabled(&window.context, MenuCommand::ManageLogbooks)
    );
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::ManageLogbooks);
    assert!(window.app.logbooks.is_open());
}

#[test]
fn queued_command_rechecks_modal_and_closing_state() {
    let mut window = Window::new();
    ready(&mut window);
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::Appearance);
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::ManageLogbooks);
    assert!(!window.app.logbooks.is_open());
    window.app.appearance.finish(
        &window.context,
        crate::ui::appearance::AppearanceAction::Cancel,
    );
    window.app.begin_close();
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::ManageLogbooks);
    assert!(!window.app.logbooks.is_open());
}

#[test]
fn menu_open_preserves_editor_draft() {
    let mut window = Window::new();
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::Appearance);
    window.app.appearance.editor.as_mut().unwrap().pending.zoom = 1.25;
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::Appearance);
    assert_eq!(
        window.app.appearance.editor.as_ref().unwrap().pending.zoom,
        1.25
    );
    window.app.appearance.finish(
        &window.context,
        crate::ui::appearance::AppearanceAction::Cancel,
    );
    assert_eq!(window.app.appearance.committed.zoom, 1.0);
}

#[test]
fn menu_open_without_focus_is_valid() {
    let mut window = Window::new();
    assert!(window.context.memory(|m| m.focused()).is_none());
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::Appearance);
    window.app.appearance.finish(
        &window.context,
        crate::ui::appearance::AppearanceAction::Cancel,
    );
    assert!(window.context.memory(|m| m.focused()).is_none());
}

#[test]
fn menu_quit_uses_existing_shutdown_once() {
    let mut window = Window::new();
    window.app.dispatch_menu(&window.context, MenuCommand::Quit);
    let started = window.app.close_started;
    window.app.dispatch_menu(&window.context, MenuCommand::Quit);
    window.app.update_lifecycle(&window.context);
    assert_eq!(window.app.close_started, started);
    assert!(matches!(
        window.commands.try_recv(),
        Ok(SessionCommand::BeginShutdown)
    ));
    window.app.update_lifecycle(&window.context);
    assert!(window.commands.try_recv().is_err());
}

#[test]
fn refresh_menu_uses_existing_request_path() {
    let mut window = Window::new();
    ready(&mut window);
    let (sender, mut requests) = mpsc::channel(8);
    window.app.settings_requests = sender;
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::RefreshSettings);
    let request = requests.try_recv().expect("actual refresh request");
    assert!(matches!(
        request.action,
        crate::session::settings::SettingsAction::Refresh
    ));
    // A second activation cannot enqueue another request while the first is pending.
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::RefreshSettings);
    assert!(requests.try_recv().is_err());
}

#[test]
fn busy_management_commands_do_not_mutate_state() {
    let mut window = Window::new();
    ready(&mut window);
    let (sender, mut requests) = mpsc::channel(8);
    window.app.settings_requests = sender;
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::RefreshSettings);
    let _pending = requests.try_recv().unwrap();
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::StationProfiles);
    assert!(!window.app.settings.is_open());
}

#[test]
fn diagnostics_menu_discloses_existing_details() {
    use egui_kittest::kittest::Queryable;
    let mut window = Window::new();
    ready(&mut window);
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::Diagnostics);
    let mut ui = main_screen::mount(window);
    assert!(ui.query_by_label_contains("Diagnostic logs:").is_some());
    assert!(ui.query_by_label_contains("Account:").is_some());
    ui.get_by_label("Done").click();
    ui.run();
    assert!(ui.query_by_label_contains("Diagnostic logs:").is_none());
}

#[test]
fn remaining_commands_keep_existing_destinations() {
    for command in [
        MenuCommand::EquipmentConfigurations,
        MenuCommand::Connection,
        MenuCommand::ResetLayout,
    ] {
        let mut window = Window::new();
        ready(&mut window);
        window.app.dispatch_menu(&window.context, command);
        match command {
            MenuCommand::EquipmentConfigurations => assert!(window.app.settings.is_open()),
            MenuCommand::Connection => assert!(window.app.enrollment.is_open()),
            MenuCommand::ResetLayout => assert!(!window.app.settings.is_open()),
            _ => unreachable!(),
        }
    }
}

#[test]
#[cfg(any(target_os = "macos", windows))]
fn toolbar_retains_logbook_and_conflict_access() {
    use egui_kittest::kittest::Queryable;
    let mut window = Window::new();
    ready(&mut window);
    let ui = main_screen::mount(window);
    assert!(ui.query_by_label("Unsynchronized changes").is_some());
    assert!(ui.query_by_label("Logbook").is_some());
    assert!(ui.query_by_label("Appearance…").is_none());
    assert!(ui.query_by_label("Manage logbooks…").is_none());
    assert!(ui.query_by_label("Saved settings").is_none());
    assert!(ui.query_by_label("View").is_none());
    assert!(
        ui.query_by_label_contains("Local workspace ready")
            .is_some()
    );
}

#[test]
fn menu_dialog_restores_previous_focus() {
    use egui_kittest::{Harness, kittest::Queryable};
    let mut ui = Harness::builder()
        .with_size(egui::vec2(900.0, 900.0))
        .build_ui_state(
            |ui, state: &mut (Window, String)| {
                let label = ui.label("Contact input");
                ui.text_edit_singleline(&mut state.1).labelled_by(label.id);
                state.0.app.appearance.show(ui.ctx());
            },
            (Window::new(), "unfinished contact".into()),
        );
    ui.get_by_label("Contact input").focus();
    ui.run();
    let context = ui.ctx.clone();
    ui.state_mut()
        .0
        .app
        .dispatch_menu(&context, MenuCommand::Appearance);
    ui.run();
    ui.get_by_label("Cancel").click();
    ui.run();
    assert!(ui.get_by_label("Contact input").is_focused());
    assert_eq!(ui.state().1, "unfinished contact");
}

#[test]
fn menu_metrics_count_dispatch_not_persistence() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        let mut window = Window::new();
        window
            .app
            .dispatch_menu(&window.context, MenuCommand::Appearance);
        window
            .app
            .dispatch_menu(&window.context, MenuCommand::ManageLogbooks);
    });
    let values = snapshotter.snapshot().into_vec();
    let entries: Vec<_> = values
        .iter()
        .filter(|(key, _, _, _)| key.key().name() == "clacks.desktop.menu.commands")
        .collect();
    assert_eq!(entries.len(), 2);
    for (key, _, _, value) in entries {
        assert_eq!(*value, DebugValue::Counter(1));
        let labels: std::collections::BTreeMap<_, _> = key
            .key()
            .labels()
            .map(|label| (label.key(), label.value()))
            .collect();
        assert_eq!(labels.len(), 2);
        match labels["command"] {
            "appearance" => assert_eq!(labels["outcome"], "accepted"),
            "manage_logbooks" => assert_eq!(labels["outcome"], "stale_disabled"),
            other => panic!("unexpected command metric {other}"),
        }
    }
}

#[test]
fn menu_quit_waits_for_pending_query() {
    use crate::session::settings::{
        SettingsAction, SettingsResponse,
        contacts::{ContactAction, ContactResponse},
    };
    use egui_kittest::kittest::Queryable;
    let mut window = Window::new();
    ready(&mut window);
    let (sender, mut requests) = mpsc::channel(16);
    window.app.settings_requests = sender;
    let mut ui = main_screen::mount(window);
    let restore = requests.try_recv().unwrap();
    assert!(matches!(
        restore.action,
        SettingsAction::Contacts(ContactAction::LoadQueryState)
    ));
    restore
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::QueryState(
            Default::default(),
        )))
        .unwrap();
    ui.step();
    // Page retrieval is irrelevant to saving the real history-expression preference.
    while let Ok(request) = requests.try_recv() {
        assert!(matches!(
            request.action,
            SettingsAction::Contacts(ContactAction::Page { .. })
        ));
        request
            .reply
            .send(SettingsResponse::Contacts(ContactResponse::Page(
                clacks_client_store::read::contacts::ContactPage {
                    rows: vec![],
                    has_more: false,
                    matching_total: 0,
                    selected_total: 0,
                    selected_outside_filter: 0,
                },
            )))
            .unwrap();
    }
    ui.get_by_label("Filter contacts").click();
    ui.get_by_label("Filter contacts").type_text("mode:SSB");
    ui.step();
    let context = ui.ctx.clone();
    // Initial UI frames also request font discovery on the lifecycle channel.
    while ui.state_mut().commands.try_recv().is_ok() {}
    ui.state_mut()
        .app
        .dispatch_menu(&context, MenuCommand::Quit);
    ui.state_mut().app.update_lifecycle(&context);
    assert!(ui.state_mut().commands.try_recv().is_err());
    let save = requests.try_recv().unwrap();
    let SettingsAction::Contacts(ContactAction::SaveQueryState { state }) = save.action else {
        panic!("expected query persistence");
    };
    assert_eq!(state.edited, "mode:SSB");
    save.reply
        .send(SettingsResponse::Contacts(
            ContactResponse::QueryStateSaved(state),
        ))
        .unwrap();
    ui.state_mut().app.update_lifecycle(&context);
    assert!(matches!(
        ui.state_mut().commands.try_recv(),
        Ok(SessionCommand::BeginShutdown)
    ));
}

#[test]
fn linux_menus_route_management_commands() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    let mut ui = Harness::builder().build_ui_state(
        |ui, window: &mut Window| {
            window.app.show_menus(ui);
            window.app.appearance.show(ui.ctx());
        },
        Window::new(),
    );
    ui.get_by_label("Settings").click();
    ui.run();
    assert!(
        ui.get_by_label("Manage logbooks…")
            .accesskit_node()
            .is_disabled()
    );
    ui.get_by_label("Appearance…").click();
    ui.run();
    assert!(ui.state().app.appearance.editor.is_some());
    ui.get_by_label("Cancel").click();
    ui.run();
    assert!(ui.state().app.appearance.editor.is_none());
}

#[test]
#[cfg(any(target_os = "macos", windows))]
fn completed_request_reenables_menu_without_a_painted_frame() {
    let mut window = Window::new();
    ready(&mut window);
    let (sender, mut requests) = mpsc::channel(8);
    window.app.settings_requests = sender;
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::RefreshSettings);
    let pending = requests.try_recv().unwrap();
    pending
        .reply
        .send(crate::session::settings::SettingsResponse::Refresh(
            fixtures::snapshot(),
        ))
        .unwrap();
    window.app.process_menus(&window.context);
    assert!(
        window
            .app
            .menu_enabled(&window.context, MenuCommand::StationProfiles)
    );
}

#[test]
#[cfg(any(target_os = "macos", windows))]
fn current_native_close_rejects_queued_management_command() {
    use eframe::App;
    let mut window = Window::new();
    ready(&mut window);
    let (settings, mut requests) = mpsc::channel(8);
    window.app.settings_requests = settings;
    let (sender, receiver) = std::sync::mpsc::channel();
    window.app.menu_receiver = Some(receiver);
    sender.send(MenuCommand::RefreshSettings).unwrap();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    let _ = window.context.run_logic(&input, |context| {
        window
            .app
            .logic(context, &mut eframe::Frame::_new_kittest());
    });
    assert!(
        requests.try_recv().is_err(),
        "closing must not admit queued refresh"
    );
    assert!(matches!(
        window.commands.try_recv(),
        Ok(SessionCommand::BeginShutdown)
    ));
}

#[test]
fn pending_contact_open_blocks_other_modal() {
    let mut window = Window::new();
    ready(&mut window);
    let (sender, mut requests) = mpsc::channel(8);
    window
        .app
        .workbench
        .open_contact(Default::default(), &sender);
    let _pending = requests.try_recv().expect("saved contact load");
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::Appearance);
    assert!(
        window.app.appearance.editor.is_none(),
        "pending contact editor reserves modal input"
    );
}

#[test]
fn reset_menu_restores_panes_without_discarding_contact() {
    use eframe::App;
    use egui_kittest::kittest::Queryable;
    #[derive(serde::Serialize, serde::Deserialize)]
    struct PaneSnapshot {
        height: f32,
        width: f32,
    }
    let mut window = Window::new();
    ready(&mut window);
    let mut storage = Storage::default();
    eframe::set_value(
        &mut storage,
        "clacks.workbench.panes.v1",
        &PaneSnapshot {
            height: 500.0,
            width: 800.0,
        },
    );
    window.app.workbench.restore(Some(&storage));
    let mut ui = main_screen::mount(window);
    ui.get_by_role_and_label(egui::Role::TextInput, "Callsign")
        .focus();
    ui.get_by_role_and_label(egui::Role::TextInput, "Callsign")
        .type_text("OK2CQR");
    ui.step();
    let context = ui.ctx.clone();
    ui.state_mut()
        .app
        .dispatch_menu(&context, MenuCommand::ResetLayout);
    ui.state_mut().app.save(&mut storage);
    let saved: PaneSnapshot = eframe::get_value(&storage, "clacks.workbench.panes.v1").unwrap();
    assert_eq!(saved.height, 350.0);
    assert_eq!(saved.width, 610.0);
    assert_eq!(ui.state().app.workbench.entry.contact_call(), "OK2CQR");
}

#[test]
#[cfg(any(target_os = "macos", windows))]
fn native_menu_text_preserves_literal_ampersand() {
    assert_eq!(
        crate::menus::native::item_text(MenuCommand::Diagnostics, false),
        "Workspace && diagnostics…"
    );
    assert_eq!(
        MenuCommand::Diagnostics.label(false),
        "Workspace & diagnostics…"
    );
}

#[test]
fn ws_suite_menu_uses_shared_modal_admission() {
    let mut window = Window::new();
    assert!(
        !window
            .app
            .menu_enabled(&window.context, MenuCommand::WsSuite)
    );
    ready(&mut window);
    window
        .app
        .dispatch_menu(&window.context, MenuCommand::WsSuite);
    assert!(window.app.ws_suite.is_open());
    assert!(
        !window
            .app
            .menu_enabled(&window.context, MenuCommand::ManageLogbooks)
    );
    assert!(
        !window
            .app
            .menu_enabled(&window.context, MenuCommand::WsSuite)
    );
}
