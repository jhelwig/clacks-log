use super::*;
use crate::session::settings::SettingsAction;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn review_sections_request_independent_pages() {
    let (requests, mut received) = mpsc::channel(8);
    let mut view = ReviewUi::default();
    let cursor = clacks_sync::record::RecordId::Qso(Default::default());
    view.conflicts.next = Some(cursor);
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1000.0, 650.0))
        .build_ui_state(|ui, view| view.show_contents(ui, &requests), view);
    ui.get_by_label("Load more conflicts").click();
    ui.step();
    let request = received.try_recv().expect("real read request");
    let SettingsAction::Review(ReviewAction::ReadPage { query }) = request.action else {
        panic!("section read")
    };
    assert_eq!(query.section, Some(ReviewSection::Conflicts));
    assert_eq!(query.after, Some(cursor));
    assert!(query.kind.is_none());
    assert!(ui.state().pending_sync.rows.is_empty());
}

#[test]
fn originating_outcomes_preserve_stale_drafts_and_survive_window_close() -> anyhow::Result<()> {
    use crate::review::{
        comparison::ComparisonSource,
        fields::{FieldId, QsoField},
        input::FieldInput,
    };
    use crate::session::settings::review::ReviewResponse;
    use clacks_client_store::save::SaveOutcome;
    let basis = crate::review::tests::basis();
    let key = basis.record.key();
    let mut view = ReviewUi {
        open: true,
        selected: Some(key),
        ..Default::default()
    };
    let mut editor = comparison::ReviewEditor::new(basis.clone())?;
    editor.draft.select_all(ComparisonSource::Local)?;
    editor.draft.set_custom(
        FieldId::Qso(QsoField::Comment),
        FieldInput::Text {
            value: "My combined comment".into(),
        },
    );
    view.drafts.insert(key, editor);
    assert!(
        matches!(view.drafts.get(&key).unwrap().draft.input(FieldId::Qso(QsoField::Comment))?, FieldInput::Text { value } if value == "My combined comment")
    );
    let (reply, response) = oneshot::channel();
    view.reply = Some(response);
    view.saving = Some(key);
    reply
        .send(SettingsResponse::Review(ReviewResponse::Resolved {
            key,
            outcome: Box::new(SaveOutcome::Stale {
                current: Some(basis.record.clone()),
            }),
        }))
        .unwrap();
    view.poll();
    assert!(view.drafts.get(&key).unwrap().draft.is_stale());
    assert!(
        view.drafts
            .get(&key)
            .unwrap()
            .draft
            .build(&Default::default())
            .is_err()
    );
    let (reply, response) = oneshot::channel();
    view.reply = Some(response);
    view.close();
    reply
        .send(SettingsResponse::RefreshFailed {
            outcome: Box::new(SettingsResponse::Review(ReviewResponse::Resolved {
                key,
                outcome: Box::new(SaveOutcome::Saved(basis.record)),
            })),
            message: "Settings reload failed".into(),
        })
        .unwrap();
    view.poll();
    assert!(view.drafts.is_empty());
    assert!(view.saving.is_none());
    assert_eq!(view.notice.as_deref(), Some("Settings reload failed"));
    Ok(())
}

#[test]
fn comparison_edits_comment_and_membership_before_admitting_resolution() -> anyhow::Result<()> {
    use crate::review::{
        comparison::ComparisonSource,
        fields::{FieldId, QsoField},
        input::FieldInput,
    };
    use crate::ui::tests::fixtures;
    struct Host {
        editor: comparison::ReviewEditor,
        settings: clacks_client_store::settings::SettingsSnapshot,
        saved: Option<RecordState>,
    }
    let basis = crate::review::tests::basis();
    let mut editor = comparison::ReviewEditor::new(basis.clone())?;
    editor.draft.select_all(ComparisonSource::Local)?;
    let mut settings = fixtures::snapshot();
    let input = editor.draft.input(FieldId::Qso(QsoField::Tags))?;
    let FieldInput::Membership { tags } = input else {
        panic!("membership")
    };
    for (index, id) in tags.iter().enumerate() {
        settings
            .logbooks
            .tags
            .push(fixtures::row(RecordValue::Tag(Box::new(
                clacks_model::tag::LogbookTag::new(*id, format!("Logbook {index}")),
            ))));
    }
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1200.0, 850.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                if let comparison::ComparisonAction::Save { chosen } =
                    host.editor
                        .show(ui, &host.settings, &Default::default(), false)
                {
                    host.saved = Some(chosen);
                }
            },
            Host {
                editor,
                settings,
                saved: None,
            },
        );
    ui.get_by_label("Resolved Comment").focus();
    ui.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    ui.get_by_label("Resolved Comment")
        .type_text("Combined on-air notes");
    ui.run();
    ui.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Logbook 0")
        .click();
    ui.run();
    let now = ui.ctx.input(|input| input.time);
    ui.input_mut().time = Some(now + 0.31);
    ui.step();
    ui.get_by_label("Save resolution").click();
    ui.run();
    let Some(RecordState::Live(RecordValue::Qso(saved))) = &ui.state().saved else {
        panic!("chosen contact")
    };
    assert_eq!(saved.comment().as_deref(), Some("Combined on-air notes"));
    assert_eq!(saved.tags().len(), tags.len() - 1);
    Ok(())
}
