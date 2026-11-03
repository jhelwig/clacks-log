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
    let mut fields = original.into_fields();
    fields.pending.as_mut().unwrap().working = RecordState::Deleted;
    let deleted: clacks_sync::client::ClientRecord = fields.try_into().unwrap();
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
        selected: [qso.id()].into(),
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

#[test]
fn editor_membership_uses_the_existing_save_boundary() {
    use egui_kittest::kittest::Queryable;
    struct Host {
        view: ContactsUi,
        snapshot: SettingsSnapshot,
        requests: mpsc::Sender<SettingsRequest>,
        definitions: DefinitionSet,
    }
    let record = fixtures::row(RecordValue::Qso(Box::new(fixtures::contact())));
    let tag =
        clacks_model::tag::LogbookTag::new(clacks_model::ids::TagId::new(), "Field day".into());
    let mut snapshot = fixtures::snapshot();
    snapshot
        .logbooks
        .tags
        .push(fixtures::row(RecordValue::Tag(Box::new(tag.clone()))));
    let mut editor = editor::ContactEditor::from_record(record.clone()).unwrap();
    editor.form.comment = "Corrected fact".into();
    let (requests, mut receiver) = mpsc::channel(32);
    let host = Host {
        view: ContactsUi {
            editor: Some(editor),
            ..Default::default()
        },
        snapshot,
        requests,
        definitions: DefinitionSet::new(vec![]).unwrap(),
    };
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1100.0, 1000.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                host.view
                    .show(ui, &host.snapshot, &host.requests, &host.definitions);
            },
            host,
        );
    ui.get_by_label("Edit…").click();
    ui.run();
    ui.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Field day")
        .click();
    ui.run();
    ui.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Field day")
        .focus();
    ui.key_press(egui::Key::Enter);
    ui.run();
    assert!(ui.state().view.editor.is_some());
    if !ui
        .state()
        .view
        .editor
        .as_ref()
        .unwrap()
        .form
        .tags
        .contains(&tag.id)
    {
        ui.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Field day")
            .click();
        ui.run();
    }
    ui.key_press(egui::Key::Escape);
    ui.run();
    assert!(ui.state().view.editor.is_some());
    assert_eq!(
        ui.state().view.editor.as_ref().unwrap().form.tags,
        [tag.id].into()
    );
    while let Ok(request) = receiver.try_recv() {
        assert!(!matches!(
            request.action,
            SettingsAction::Contacts(ContactAction::Save { .. })
        ));
    }
    ui.get_by_label("Cancel").scroll_to_me();
    ui.run();
    ui.get_by_label("Cancel").click();
    ui.run();
    assert!(ui.state().view.editor.is_none());
    assert!(matches!(
        receiver.try_recv().unwrap().action,
        SettingsAction::Directory(crate::directory::lookup::LookupCommand::Deleted { .. })
    ));
    let mut editor = editor::ContactEditor::from_record(record.clone()).unwrap();
    editor.form.comment = "Corrected fact".into();
    ui.state_mut().view.editor = Some(editor);
    ui.run();
    ui.get_by_label("Edit…").scroll_to_me();
    ui.run();
    ui.get_by_label("Edit…").click();
    ui.run();
    ui.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Field day")
        .click();
    ui.run();
    ui.key_press(egui::Key::Escape);
    ui.run();
    let now = ui.ctx.input(|input| input.time);
    ui.input_mut().time = Some(now + 0.31);
    ui.step();
    assert!(ui.state().view.editor.as_ref().unwrap().errors.is_empty());
    assert!(
        ui.state()
            .view
            .editor
            .as_ref()
            .unwrap()
            .validation
            .can_submit(&[])
    );
    assert!(ui.state().view.operation.is_none());
    ui.get_by_label("Save changes").scroll_to_me();
    ui.run();
    ui.get_by_label("Save changes").click();
    ui.run();
    let request = receiver.try_recv().unwrap();
    let SettingsAction::Contacts(ContactAction::Save { expected, next, .. }) = request.action
    else {
        panic!("save")
    };
    assert_eq!(expected.as_deref(), Some(&record));
    assert_eq!(next.tags(), &std::collections::BTreeSet::from([tag.id]));
    assert_eq!(next.comment().as_deref(), Some("Corrected fact"));
}

#[test]
fn history_membership_request_uses_all_selected_ids() {
    use crate::session::settings::contacts::membership::MembershipBasis;
    use clacks_client_store::save::tags::{TagChangeEntry, TagChangeReport, TagChangeStop};
    use egui_kittest::kittest::Queryable;
    struct Host {
        view: ContactsUi,
        snapshot: SettingsSnapshot,
        requests: mpsc::Sender<SettingsRequest>,
        definitions: DefinitionSet,
    }
    let ids: BTreeSet<QsoId> = [QsoId::new(), QsoId::new(), QsoId::new()].into();
    let (requests, mut receiver) = mpsc::channel(32);
    let host = Host {
        view: ContactsUi {
            selected: ids.clone(),
            ..Default::default()
        },
        snapshot: fixtures::snapshot(),
        requests,
        definitions: DefinitionSet::new(vec![]).unwrap(),
    };
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                host.view
                    .show(ui, &host.snapshot, &host.requests, &host.definitions);
            },
            host,
        );
    ui.get_by_label("Edit assigned logbooks…").click();
    ui.run();
    let mut prepare = None;
    while let Ok(request) = receiver.try_recv() {
        if let SettingsAction::Contacts(ContactAction::PrepareMembership { ids: submitted }) =
            &request.action
        {
            assert_eq!(submitted, &ids);
            prepare = Some(request.reply);
        }
    }
    let tag =
        clacks_model::tag::LogbookTag::new(clacks_model::ids::TagId::new(), "Portable".into());
    let records: Vec<_> = ids
        .iter()
        .map(|id| {
            let mut qso = fixtures::contact();
            crate::test_support::edit(&mut qso, |qso| {
                qso.id = *id;
                Ok(())
            });
            fixtures::row(RecordValue::Qso(Box::new(qso)))
        })
        .collect();
    let mut catalog = fixtures::snapshot().logbooks;
    catalog.tags = vec![fixtures::row(RecordValue::Tag(Box::new(tag.clone())))];
    prepare
        .unwrap()
        .send(SettingsResponse::Contacts(
            ContactResponse::MembershipPrepared(MembershipBasis {
                records: records.clone(),
                catalog,
            }),
        ))
        .unwrap();
    ui.run();
    ui.get_by_label("Portable").click();
    ui.run();
    ui.get_by_label("Apply").click();
    ui.run();
    let request = receiver.try_recv().unwrap();
    let SettingsAction::Contacts(ContactAction::ChangeMembership(submission)) = request.action
    else {
        panic!("assignment")
    };
    assert_eq!(submission.basis, records);
    assert_eq!(submission.add, [tag.id].into());
    assert!(submission.remove.is_empty());
    ui.state_mut()
        .view
        .refresh(clacks_client_store::logbooks::selection::SelectedView::Untagged);
    ui.state_mut().view.selected.clear();
    ui.run();
    assert!(ui.query_by_label("Edit assigned logbooks…").is_none());
    ui.get_by_label("Cancel processing").click();
    ui.run();
    assert!(submission.cancel.is_cancelled());
    let mut rows = records.into_iter();
    let first = rows.next().unwrap();
    let second = rows.next().unwrap();
    let third = rows.next().unwrap();
    let clacks_sync::record::RecordId::Qso(first_id) = first.key().id else {
        panic!("qso")
    };
    let clacks_sync::record::RecordId::Qso(second_id) = second.key().id else {
        panic!("qso")
    };
    let clacks_sync::record::RecordId::Qso(third_id) = third.key().id else {
        panic!("qso")
    };
    request
        .reply
        .send(SettingsResponse::RefreshFailed {
            outcome: Box::new(SettingsResponse::Contacts(
                ContactResponse::MembershipChanged(TagChangeReport {
                    entries: vec![
                        TagChangeEntry {
                            id: first_id,
                            outcome: SaveOutcome::Saved(first),
                        },
                        TagChangeEntry {
                            id: second_id,
                            outcome: SaveOutcome::Stale {
                                current: Some(second),
                            },
                        },
                    ],
                    stop: Some(TagChangeStop::Cancelled),
                    unprocessed: vec![third_id],
                }),
            )),
            message: "Refresh failed after local changes".into(),
        })
        .unwrap();
    ui.run();
    assert!(
        ui.query_by_label_contains("1 saved · 0 unchanged · 1 stale · 1 unprocessed")
            .is_some()
    );
    assert!(
        ui.query_by_label_contains("Refresh failed after local changes")
            .is_some()
    );
    assert!(ui.state().view.changed);
    ui.get_by_label("Assignment results").click();
    ui.run();
    assert!(
        ui.query_by_label_contains(&format!("{third_id} · Unprocessed"))
            .is_some()
    );
    ui.get_by_label("Dismiss assignment results").click();
    ui.run();
    ui.state_mut().view.selected = ids;
    ui.run();
    ui.get_by_label("Edit assigned logbooks…").click();
    ui.run();
    let mut late = None;
    while let Ok(request) = receiver.try_recv() {
        if matches!(
            request.action,
            SettingsAction::Contacts(ContactAction::PrepareMembership { .. })
        ) {
            late = Some(request.reply);
        }
    }
    ui.state_mut()
        .view
        .refresh(clacks_client_store::logbooks::selection::SelectedView::Untagged);
    ui.run();
    assert!(
        late.unwrap()
            .send(SettingsResponse::Failed("obsolete read".into()))
            .is_err()
    );
    assert!(ui.state().view.membership.is_none());
}

#[test]
fn directory_opening_saved_contact_does_not_refresh_its_unchanged_call() {
    let record = crate::ui::tests::fixtures::row(clacks_sync::record::RecordValue::Qso(Box::new(
        fixtures::contact(),
    )));
    let mut editor = editor::ContactEditor::from_record(record).unwrap();
    let (sender, mut requests) = tokio::sync::mpsc::channel(4);
    editor
        .directory
        .observe(editor.id().unwrap(), &editor.form.dx.callsign, &sender);
    assert!(requests.try_recv().is_err());
    editor.form.dx.callsign = "W7OTHER".into();
    editor
        .directory
        .observe(editor.id().unwrap(), &editor.form.dx.callsign, &sender);
    assert!(matches!(
        requests.try_recv().unwrap().action,
        SettingsAction::Directory(crate::directory::lookup::LookupCommand::CallChanged { .. })
    ));
}

#[test]
fn directory_editor_close_keeps_its_notification_until_the_session_admits_it() {
    let mut view = ContactsUi::default();
    let qso = clacks_model::ids::QsoId::new();
    view.closed_editor = Some(qso);
    let (sender, mut requests) = tokio::sync::mpsc::channel(1);
    sender
        .try_send(SettingsRequest {
            action: SettingsAction::Refresh,
            reply: tokio::sync::oneshot::channel().0,
        })
        .unwrap();
    assert!(!view.flush_closed_editor(&sender));
    assert_eq!(view.closed_editor, Some(qso));
    requests.try_recv().unwrap();
    assert!(view.flush_closed_editor(&sender));
    assert!(view.closed_editor.is_none());
    assert!(
        matches!(requests.try_recv().unwrap().action, SettingsAction::Directory(crate::directory::lookup::LookupCommand::Deleted { qso: owner }) if owner == qso)
    );
}

#[test]
fn saved_editor_displays_directory_name_without_recording_it_on_save() {
    use clacks_model::directory::values::{
        DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields, LookupKey,
    };
    use egui_kittest::kittest::{NodeT, Queryable};
    let mut contact = fixtures::contact();
    crate::test_support::edit(&mut contact, |fields| {
        fields.contacted_operator = None;
        fields.contacted_station.location = None;
        Ok(())
    });
    let mut editor = editor::ContactEditor::from_record(fixtures::row(RecordValue::Qso(Box::new(
        contact.clone(),
    ))))
    .unwrap();
    editor.directory.retain(Some(Box::new(
        DirectorySnapshot::new(DirectorySnapshotFields {
            provider: DirectoryProvider::Qrz,
            key: LookupKey::from_recorded(&contact.contacted_station().callsign),
            lookup_time: clacks_model::scalar::UtcTimestamp::new(1, 0).unwrap(),
            lookup_id: clacks_model::ids::LookupId::new(),
            facts: DirectoryFacts {
                name: Some("Retained directory name".into()),
                ..Default::default()
            },
        })
        .unwrap(),
    )));
    let view = ContactsUi {
        editor: Some(editor),
        ..Default::default()
    };
    let (sender, mut requests) = mpsc::channel(8);
    let catalog = fixtures::snapshot().logbooks;
    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1400.0, 1600.0))
        .build_ui_state(
            move |ui, view: &mut ContactsUi| {
                view.show_editor(ui.ctx(), &DefinitionSet::default(), &sender, &catalog)
            },
            view,
        );
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::TextInput, "Name")
            .accesskit_node()
            .data()
            .placeholder(),
        Some("Retained directory name")
    );
    assert!(
        ui.query_by_label("QRZ name: Retained directory name")
            .is_none()
    );
    ui.get_by_label("Save changes").scroll_to_me();
    ui.run();
    ui.get_by_label("Save changes").click();
    ui.run();
    let request = requests.try_recv().unwrap();
    let SettingsAction::Contacts(ContactAction::Save {
        next,
        snapshot: Some(snapshot),
        ..
    }) = request.action
    else {
        panic!("save with retained directory snapshot")
    };
    assert!(next.contacted_operator().is_none());
    assert!(next.contacted_station().location.is_none());
    assert_eq!(
        snapshot.facts().name.as_deref(),
        Some("Retained directory name")
    );
}
