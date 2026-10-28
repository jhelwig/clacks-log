//! Contact review reacts to native outcomes without re-testing store transactions.

use super::*;
use crate::ui::tests::fixtures;
use clacks_sync::record::{RecordState, RecordValue};
use egui_kittest::Harness;

#[test]
fn stale_save_preserves_the_correction_and_offers_review_without_resurrection() {
    let original = fixtures::row(RecordValue::Qso(Box::new(fixtures::contact())));
    let mut editor = editor::ContactEditor::from_record(original.clone()).unwrap();
    editor.form.comment = "My unsaved correction".into();
    let mut deleted = original;
    deleted.pending.as_mut().unwrap().working = RecordState::Deleted;
    let mut view = ContactsUi {
        editor: Some(editor),
        ..Default::default()
    };

    view.accept(ContactResponse::Saved(SaveOutcome::Stale {
        current: Some(deleted.clone()),
    }));

    assert_eq!(
        view.editor.as_ref().unwrap().form.comment,
        "My unsaved correction"
    );
    assert_eq!(view.current_record, Some(deleted));
    assert!(view.notice.as_ref().unwrap().contains("changed"));
    assert!(view.operation.is_none());
}

#[test]
fn enter_on_deletion_confirmation_defaults_to_cancellation() {
    let confirmation = deletion::DeletionConfirmation {
        expected: fixtures::row(RecordValue::Qso(Box::new(fixtures::contact()))),
        first_frame: true,
    };
    struct Host {
        confirmation: deletion::DeletionConfirmation,
        choice: deletion::DeletionAction,
    }
    let mut ui = Harness::builder()
        .with_size(egui::vec2(800.0, 500.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                let choice = deletion::show(ui.ctx(), &mut host.confirmation, false);
                if !matches!(choice, deletion::DeletionAction::None) {
                    host.choice = choice;
                }
            },
            Host {
                confirmation,
                choice: deletion::DeletionAction::None,
            },
        );
    ui.key_press(egui::Key::Enter);
    ui.step();
    assert!(matches!(
        ui.state().choice,
        deletion::DeletionAction::Cancel
    ));
}

#[test]
fn view_change_clears_selection_and_ignores_an_old_page_reply() {
    let qso = fixtures::contact();
    let (reply, receiver) = oneshot::channel();
    let mut view = ContactsUi {
        selected: [qso.id].into(),
        page_request: Some(PageRequest {
            query: ContactPageQuery::default(),
            expression: String::new(),
            receiver,
        }),
        ..Default::default()
    };
    view.refresh(SelectedView::Untagged);
    reply
        .send(SettingsResponse::Contacts(ContactResponse::Page(
            ContactPage {
                rows: vec![clacks_client_store::read::contacts::ContactRow {
                    facts: Default::default(),
                    working: clacks_client_store::read::history::HistoryRow {
                        qso,
                        pending: true,
                        review: None,
                    },
                }],
                has_more: false,
                matching_total: 1,
                selected_total: 1,
                selected_outside_filter: 0,
            },
        )))
        .unwrap();
    view.poll();
    assert!(view.selected.is_empty());
    assert!(view.page.is_none());
    assert_eq!(view.query.view, SelectedView::Untagged);
    assert!(view.dirty);
}

#[test]
fn query_replies_follow_current_inputs_and_failures_keep_complete_rows() -> anyhow::Result<()> {
    let (sender, mut requests) = mpsc::channel(8);
    let mut view = ContactsUi::default();
    let catalog = clacks_model::recognition::RecognitionCatalog::adif_318()?;
    view.filter.desired = Some(clacks_model::query::parse("mode:SSB", catalog)?);
    view.filter.desired_text = "mode:SSB".into();
    view.update_query();
    view.read_page(&sender);
    let old = requests.try_recv()?;
    let SettingsAction::Contacts(ContactAction::Page { query }) = &old.action else {
        anyhow::bail!("missing page")
    };
    assert_eq!(
        query.filter,
        clacks_model::query::parse("mode:SSB", catalog)?
    );

    view.filter.desired = Some(clacks_model::query::parse("mode:CW", catalog)?);
    view.filter.desired_text = "mode:CW".into();
    view.update_query();
    view.read_page(&sender);
    assert!(old.reply.is_closed());
    let current = requests.try_recv()?;
    current
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::Page(
            ContactPage {
                rows: vec![],
                matching_total: 7,
                selected_total: 0,
                selected_outside_filter: 0,
                has_more: false,
            },
        )))
        .unwrap();
    view.poll();
    assert_eq!(view.page.as_ref().unwrap().matching_total, 7);
    assert_eq!(view.filter.state.applied, "mode:CW");

    view.refresh(view.query.view);
    view.update_query();
    view.read_page(&sender);
    requests
        .try_recv()?
        .reply
        .send(SettingsResponse::Failed(
            "Cannot read current contacts".into(),
        ))
        .unwrap();
    view.poll();
    assert_eq!(view.page.as_ref().unwrap().matching_total, 7);
    assert_eq!(view.filter.state.applied, "mode:CW");
    assert!(view.problem);
    assert!(view.notice.as_ref().unwrap().contains("Cannot read"));
    Ok(())
}
