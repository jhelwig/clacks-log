//! Pending and stale outcomes remain owned by the initiating logbook workflow.

use super::*;

#[test]
fn stale_deletion_requires_reload_and_a_new_explicit_choice() {
    let mut ui = mount();
    let preview = deletion_review(&mut ui);
    ui.get_by_label("Delete all contacts (delete 2)").click();
    ui.run();
    ui.get_by_label("Confirm deletion").click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    request
        .reply
        .send(SettingsResponse::Logbooks(LogbookResponse::Deleted(
            clacks_client_store::logbooks::deletion::TagDeleteOutcome::Stale {
                current: Some(Box::new(preview.tag.clone())),
            },
        )))
        .unwrap();
    ui.run();
    ui.get_by_label("Reload deletion review").click();
    ui.step();
    let reload = ui.state_mut().requests.try_recv().unwrap();
    assert!(matches!(
        reload.action,
        SettingsAction::Logbooks(LogbookAction::PreviewDeletion { .. })
    ));
    reload
        .reply
        .send(SettingsResponse::Logbooks(
            LogbookResponse::DeletionPreview(preview),
        ))
        .unwrap();
    ui.run();
    assert!(
        ui.get_by_label("Confirm deletion")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn dismissed_request_keeps_other_actions_blocked_until_its_reply_arrives() {
    let mut ui = mount();
    deletion_review(&mut ui);
    ui.get_by_label("Leave all contacts (delete 0)").click();
    ui.run();
    ui.get_by_label("Confirm deletion").click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    ui.key_press(egui::Key::Escape);
    ui.step();
    assert!(
        ui.get_by_label("Manage logbooks…")
            .accesskit_node()
            .is_disabled()
    );
    request
        .reply
        .send(SettingsResponse::Failed("Could not delete logbook".into()))
        .unwrap();
    ui.run();
    assert!(
        !ui.get_by_label("Manage logbooks…")
            .accesskit_node()
            .is_disabled()
    );
    assert!(ui.query_by_label("Could not delete logbook").is_some());
    assert!(ui.state_mut().requests.try_recv().is_err());
}

#[test]
fn deleted_view_fallback_is_reported_once_with_safe_context() -> anyhow::Result<()> {
    let tag = LogbookTag::new(Default::default(), "Private logbook name".into());
    let record = fixtures::row(RecordValue::Tag(Box::new(tag.clone())));
    let account = record.key().account;
    let before = LogbookSnapshot {
        selected_view: SelectedView::Logbook(tag.id),
        tags: vec![record.clone()],
    };
    let mut fields = record.into_fields();
    fields.pending.as_mut().unwrap().working = RecordState::Deleted;
    let deleted: clacks_sync::client::ClientRecord = fields.try_into().unwrap();
    let after = LogbookSnapshot {
        selected_view: SelectedView::AllContacts,
        tags: vec![deleted],
    };
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::sync::Mutex::new(output.reopen()?))
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let mut view = LogbooksUi::default();
        view.observe(&before, None);
        view.observe(&after, None);
        view.fallback_notice = None;
        view.observe(&after, None);
    });
    let logged = std::fs::read_to_string(output.path())?;
    assert_eq!(
        logged
            .lines()
            .filter(|line| line.contains("logbook.view_fallback_observed"))
            .count(),
        1
    );
    assert!(logged.contains(&account.to_string()));
    assert!(logged.contains(&tag.id.to_string()));
    assert!(logged.contains("deleted_tag"));
    assert!(!logged.contains(&tag.name));
    Ok(())
}
