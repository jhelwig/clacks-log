//! Logbook components are checked at their native request/response boundary.

mod responses;

use super::*;
use crate::ui::tests::fixtures;
use clacks_client_store::logbooks::deletion::{TagDeletionCounts, TagDeletionPreview};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::sync::Arc;

struct Host {
    view: LogbooksUi,
    snapshot: LogbookSnapshot,
    sender: mpsc::Sender<SettingsRequest>,
    requests: mpsc::Receiver<SettingsRequest>,
}

fn mount() -> Harness<'static, Host> {
    let (sender, requests) = mpsc::channel(8);
    let tag = LogbookTag::new(Default::default(), "Main".into());
    Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1100.0, 900.0))
        .build_ui_state(
            |ui, host| {
                host.view.observe(&host.snapshot, None);
                host.view.show(ui, &host.snapshot, &host.sender);
                host.view.feedback(ui, &host.sender);
                host.view.dialog(ui.ctx(), &host.snapshot, &host.sender);
            },
            Host {
                view: Default::default(),
                snapshot: LogbookSnapshot {
                    selected_view: SelectedView::AllContacts,
                    tags: vec![fixtures::row(RecordValue::Tag(Box::new(tag)))],
                },
                sender,
                requests,
            },
        )
}

fn deletion_review(ui: &mut Harness<'_, Host>) -> Arc<TagDeletionPreview> {
    ui.get_by_label("Manage logbooks…").click();
    ui.run();
    ui.get_by_label("Delete logbook").click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    let SettingsAction::Logbooks(LogbookAction::PreviewDeletion { id }) = request.action else {
        panic!()
    };
    let mut unique = fixtures::contact();
    crate::test_support::edit(&mut unique, |unique| {
        unique.tags = [id].into();
        Ok(())
    });
    let mut shared = unique.clone();
    crate::test_support::edit(&mut shared, |shared| {
        shared.id = Default::default();
        Ok(())
    });
    crate::test_support::edit(&mut shared, |shared| {
        shared.tags.insert(TagId::new());
        Ok(())
    });
    let preview = Arc::new(TagDeletionPreview {
        tag: ui.state().snapshot.tags.first().unwrap().clone(),
        members: vec![
            fixtures::row(RecordValue::Qso(Box::new(unique))),
            fixtures::row(RecordValue::Qso(Box::new(shared))),
        ],
        counts: TagDeletionCounts {
            affected: 2,
            also_tagged_elsewhere: 1,
            unique: 1,
        },
    });
    request
        .reply
        .send(SettingsResponse::Logbooks(
            LogbookResponse::DeletionPreview(preview.clone()),
        ))
        .unwrap();
    ui.run();
    preview
}

#[test]
fn edited_logbook_submits_its_identity_and_policy_without_selecting_it() {
    let mut ui = mount();
    let before = ui.state().snapshot.clone();
    ui.get_by_label("Manage logbooks…").click();
    ui.run();
    ui.get_by_label("Edit logbook").click();
    ui.run();
    ui.get_by_label("Logbook name").focus();
    ui.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    ui.get_by_label("Logbook name").type_text("Renamed");
    ui.run();
    ui.get_all_by_label("Forbid").last().unwrap().click();
    ui.run();
    ui.get_by_label("Save logbook").click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    let SettingsAction::Logbooks(LogbookAction::SaveTag { expected, next }) = request.action else {
        panic!()
    };
    assert_eq!(expected.as_deref(), before.tags.first());
    assert_eq!(next.name, "Renamed");
    assert_eq!(
        next.automatic_upload.lotw,
        clacks_model::tag::AutomaticUploadPolicy::Neither
    );
    assert_eq!(
        next.automatic_upload.qrz,
        clacks_model::tag::AutomaticUploadPolicy::Forbid
    );
    assert_eq!(ui.state().snapshot.selected_view, SelectedView::AllContacts);
    assert!(ui.state_mut().requests.try_recv().is_err());
}

#[test]
fn escape_discards_a_new_tag_without_submitting_or_changing_the_view() {
    let mut ui = mount();
    ui.get_by_label("Manage logbooks…").click();
    ui.run();
    ui.get_by_label("New logbook").click();
    ui.run();
    ui.get_by_label("Logbook name").focus();
    ui.get_by_label("Logbook name").type_text("Discard me");
    ui.run();
    ui.key_press(egui::Key::Escape);
    ui.run();
    assert!(ui.state_mut().requests.try_recv().is_err());
    assert!(ui.state().view.management.is_none());
    assert!(ui.get_by_label("Manage logbooks…").is_focused());
}

#[test]
fn deletion_requires_an_explicit_choice_and_submits_the_reviewed_snapshot() {
    use clacks_model::tag::TagDeletionChoice;
    for (label, desired) in [
        ("Leave all contacts (delete 0)", TagDeletionChoice::LeaveAll),
        (
            "Delete all contacts (delete 2)",
            TagDeletionChoice::DeleteAll,
        ),
        (
            "Delete unique contacts (delete 1)",
            TagDeletionChoice::DeleteUnique,
        ),
    ] {
        let mut ui = mount();
        let preview = deletion_review(&mut ui);
        assert!(
            ui.get_by_label("Confirm deletion")
                .accesskit_node()
                .is_disabled()
        );
        ui.get_by_label("Confirm deletion").click();
        ui.step();
        assert!(ui.state_mut().requests.try_recv().is_err());
        ui.get_by_label(label).click();
        ui.run();
        ui.get_by_label("Confirm deletion").click();
        ui.step();
        let request = ui.state_mut().requests.try_recv().unwrap();
        let SettingsAction::Logbooks(LogbookAction::DeleteTag { expected, choice }) =
            request.action
        else {
            panic!()
        };
        assert_eq!(expected, preview);
        assert_eq!(choice, desired);
    }
}

#[test]
fn a_tag_named_like_a_builtin_selects_its_native_identity() {
    let mut ui = mount();
    let mut named = tag(ui.state().snapshot.tags.first().unwrap())
        .unwrap()
        .clone();
    named.name = "All contacts".into();
    ui.state_mut().snapshot.tags = vec![fixtures::row(RecordValue::Tag(Box::new(named.clone())))];
    ui.run();
    ui.get_by_label("Logbook").click();
    ui.run();
    ui.get_all_by_label("All contacts").last().unwrap().click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    assert!(
        matches!(request.action, SettingsAction::Logbooks(LogbookAction::SelectView { next: SelectedView::Logbook(id) }) if id == named.id)
    );
    assert_eq!(ui.state().snapshot.selected_view, SelectedView::AllContacts);
}
