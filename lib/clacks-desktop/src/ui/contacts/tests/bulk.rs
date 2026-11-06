//! Bulk UI owns the selected intent and report independently of history navigation.
use super::*;
use crate::directory::lookup::bulk::{BulkLookupHandle, report::*};
use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

#[test]
fn bulk_lookup_menu_uses_current_selection_and_prevents_duplicate_submission() {
    let (sender, mut requests) = mpsc::channel(8);
    let ids = [QsoId::new(), QsoId::new()].into();
    let mut view = ContactsUi {
        selected: ids,
        ..Default::default()
    };
    view.start_directory_lookup(&sender);
    view.start_directory_lookup(&sender);
    let request = requests.try_recv().unwrap();
    let SettingsAction::Contacts(ContactAction::StartDirectoryLookup { ids }) = request.action
    else {
        panic!("bulk lookup");
    };
    assert_eq!(ids, view.selected);
    assert!(requests.try_recv().is_err());
    assert!(!view.has_modal());
    assert!(view.bulk_running());
}

#[test]
fn bulk_lookup_report_survives_view_change_and_has_explicit_cancel_and_dismiss() {
    use egui_kittest::kittest::Queryable;
    let (sender, mut requests) = mpsc::channel(8);
    let id = QsoId::new();
    let mut view = ContactsUi {
        selected: [id].into(),
        ..Default::default()
    };
    view.start_directory_lookup(&sender);
    let request = requests.try_recv().unwrap();
    let cancel = CancellationToken::new();
    let (updates, receiver) = watch::channel(BulkUpdate::Preparing { total: 1 });
    request
        .reply
        .send(SettingsResponse::Contacts(
            ContactResponse::DirectoryLookupStarted(BulkLookupHandle {
                cancel: cancel.clone(),
                updates: receiver,
            }),
        ))
        .unwrap();
    view.poll_bulk_lookup();
    view.refresh(SelectedView::Untagged);
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1000.0, 600.0))
        .build_ui_state(
            |ui, view: &mut ContactsUi| {
                view.poll_bulk_lookup();
                view.bulk_feedback(ui);
            },
            view,
        );
    ui.get_by_label("Cancel lookup").click();
    ui.run();
    assert!(cancel.is_cancelled());
    updates.send_replace(BulkUpdate::Finished(Arc::new(BulkReport {
        total: 1,
        counts: BulkCounts {
            unprocessed: 1,
            ..Default::default()
        },
        details: vec![],
        stop: BulkStop::PreparationCancelled,
    })));
    ui.run();
    assert!(
        ui.query_by_label(
            "Cancelled during preparation. Lookup did not start; no bulk snapshot writes were made."
        )
        .is_some()
    );
    let failure = BulkFailure::Storage {
        message: "Workspace storage is unavailable. Completed updates were preserved; check available disk space and workspace diagnostics before retrying.".into(),
    };
    updates.send_replace(BulkUpdate::Finished(Arc::new(BulkReport {
        total: 1,
        counts: BulkCounts {
            failed: 1,
            ..Default::default()
        },
        details: vec![BulkDetail {
            contact: BulkContact {
                id,
                callsign: Some(clacks_model::scalar::RecordedCallsign::new("W7TEST").unwrap()),
                start: Some(clacks_model::scalar::UtcSecond::from_unix_seconds(1)),
            },
            outcome: BulkDetailOutcome::Failed { failure },
        }],
        stop: BulkStop::Stopped {
            reason: BulkFailure::Authentication,
        },
    })));
    ui.run();
    assert!(ui.query_by_label("Failed: 1").is_some());
    assert!(!ui.state().bulk_running());
    ui.get_by_label("Failed contacts (1)").click();
    ui.run();
    assert!(ui.query_by_label("W7TEST").is_some());
    ui.get_by_label("Dismiss lookup report").click();
    ui.run();
    assert!(ui.query_by_label("Failed: 1").is_none());
}
