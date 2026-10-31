//! Application chrome exposes diagnostics on request without hiding failures.

use super::*;
use egui_kittest::{Harness, kittest::Queryable};

pub(super) fn mount(window: Window) -> Harness<'static, Window> {
    Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui_state(
            |ui, window| {
                use eframe::App;
                window.app.ui(ui, &mut eframe::Frame::_new_kittest());
            },
            window,
        )
}

#[test]
fn workspace_details_are_disclosed_only_on_request() {
    let window = Window::new();
    window.updates.send_replace(SessionStatus::Ready {
        identity: clacks_client_store::identity::WorkspaceStatus {
            account: Default::default(),
            enrolled: false,
        },
        restored_view_from: None,
        synchronization: crate::synchronization::Status::Disconnected,
    });
    let mut ui = mount(window);
    assert!(ui.query_by_label_contains("Diagnostic logs:").is_none());
    ui.get_by_label("Workspace & diagnostics").click();
    ui.run();
    assert!(ui.query_by_label_contains("Diagnostic logs:").is_some());
    assert!(ui.query_by_label_contains("Account:").is_some());
}

#[test]
fn collapsed_diagnostics_do_not_hide_startup_or_log_output_failures() {
    let window = Window::new();
    window.updates.send_replace(SessionStatus::Failed {
        message: "Database could not be opened".into(),
    });
    window
        .app
        .file_failure
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let ui = mount(window);
    assert!(ui.query_by_label("Database could not be opened").is_some());
    assert!(ui.query_by_label("Retry").is_some());
    assert!(
        ui.query_by_label_contains("Local log file writes failed")
            .is_some()
    );
}
