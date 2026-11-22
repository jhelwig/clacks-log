//! Checks for the shared Settings action and navigation boundaries.

use super::*;
use crate::{menus::MenuCommand, session::SessionCommand, ui::tests::main_screen::mount};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn settings_keeps_one_draft_across_sections_and_dismisses_preview() {
    let mut app = mount(crate::ui::tests::Window::new());
    let context = app.ctx.clone();
    app.state_mut()
        .app
        .dispatch_menu(&context, MenuCommand::OpenSettings);
    app.run();

    let profiles = app.get_by_label("Station profiles").rect();
    let equipment = app.get_by_label("Equipment").rect();
    let appearance_option = app.get_by_label("Dark").rect();
    assert!(profiles.max.y <= equipment.min.y);
    assert!(equipment.max.x <= appearance_option.min.x);

    app.get_by_label("Dark").click();
    app.run();
    assert!(!app.get_by_label("Apply").accesskit_node().is_disabled());

    app.get_by_label("Logbooks").click();
    app.step();
    assert!(!app.get_by_label("Apply").accesskit_node().is_disabled());
    app.get_by_label("Cancel").click();
    app.run_steps(3);
    assert!(app.state().app.settings_screen.is_none());
    assert!(!app.state().app.appearance.changed());
    assert!(app.state().app.settings_intent.borrow().is_none());
}

#[test]
fn settings_apply_clears_draft_and_marks_failed_items() -> anyhow::Result<()> {
    use clacks_client_store::{identity::WorkspaceStatus, ws_suite::ReceiverSettings};
    use clacks_model::{ids::TagId, tag::LogbookTag};
    use clacks_sync::record::RecordValue;
    use std::sync::Arc;
    use tokio::sync::{mpsc, watch};

    let mut window = crate::ui::tests::Window::new();
    let identity = WorkspaceStatus {
        account: Default::default(),
        enrolled: false,
    };
    window.updates.send_replace(SessionStatus::Ready {
        identity,
        synchronization: crate::synchronization::Status::Disconnected,
        restored_view_from: None,
        settings_read_error: None,
    });
    let mut snapshot = crate::ui::tests::fixtures::snapshot();
    let tag_id = TagId::new();
    snapshot
        .logbooks
        .tags
        .push(crate::ui::tests::fixtures::row(RecordValue::Tag(Box::new(
            LogbookTag::new(tag_id, "Main".into()),
        ))));
    let (basis, receiver) = watch::channel(None);
    window.app.settings_basis = receiver;
    let (view, receiver_view) = watch::channel(crate::ws_suite::ReceiverView {
        settings: Some(ReceiverSettings {
            enabled: true,
            ..Default::default()
        }),
        ..Default::default()
    });
    window.app.ws_suite_view = receiver_view;
    let (sender, mut settings) = mpsc::channel(8);
    window.app.settings_requests = sender;
    let mut app = mount(window);
    let context = app.ctx.clone();
    app.state_mut()
        .app
        .dispatch_menu(&context, MenuCommand::OpenSettings);
    let id = (*app.state().app.settings_intent.borrow()).expect("Settings opened");
    basis.send_replace(Some(crate::session::SettingsBasis {
        id,
        identity,
        snapshot: Arc::new(snapshot.clone()),
    }));
    app.run();
    let interfaces = std::iter::from_fn(|| app.state_mut().commands.try_recv().ok())
        .find_map(|command| match command {
            SessionCommand::WsSuite(request) => Some(request),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("expected receiver interface discovery"))?;
    interfaces
        .reply
        .send(crate::ws_suite::ReceiverResponse::Interfaces(Ok(vec![])))
        .expect("Settings retains the interface result");
    app.state_mut().app.ws_suite.poll(&view.borrow());
    app.get_by_label("Main").click();
    app.run();
    app.get_by_label("Logbook name")
        .set_text_value_accesskit("Field Day");
    app.run();
    app.get_by_label("WS Suite").click();
    app.run();
    app.get_by_label("Multicast").click();
    app.run();
    app.get_by_label("Port")
        .set_text_value_accesskit("bad port");
    app.run();
    app.get_by_label("Apply").click();
    app.step();
    app.step();

    let request = settings.try_recv()?;
    let SettingsAction::ApplyAccountChanges(changes) = request.action else {
        anyhow::bail!("expected account changes");
    };
    let edit = changes
        .tags
        .get(&tag_id)
        .expect("draft contains the renamed logbook");
    assert_eq!(edit.next.name, "Field Day");
    snapshot.logbooks.tags = vec![crate::ui::tests::fixtures::row(RecordValue::Tag(Box::new(
        edit.next.clone(),
    )))];
    basis.send_replace(Some(crate::session::SettingsBasis {
        id,
        identity,
        snapshot: Arc::new(snapshot),
    }));
    request
        .reply
        .send(SettingsResponse::ApplyAccountChanges(Default::default()))
        .expect("Settings is awaiting account results");
    app.run();

    let apply = app.get_by_label("Apply, some changes could not be applied");
    assert!(apply.accesskit_node().is_disabled());
    assert!(
        app.query_by_label("WS Suite, changes could not be applied")
            .is_some()
    );
    assert!(
        app.query_by_label_contains("Use a port from 1 to 65535.")
            .is_some()
    );
    assert!(
        app.query_by_label_contains("Group address: Enter an IP address.")
            .is_some()
    );
    assert!(app.query_by_label("Close").is_some());
    app.get_by_label("Logbooks").click();
    app.run();
    assert!(app.query_by_label("Field Day").is_some());
    Ok(())
}
