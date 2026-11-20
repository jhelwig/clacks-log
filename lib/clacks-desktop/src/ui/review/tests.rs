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
fn pending_resolution_stays_open_and_retains_unsaved_drafts() -> anyhow::Result<()> {
    use crate::review::{
        comparison::ComparisonSource,
        fields::{FieldId, QsoField},
        input::FieldInput,
    };
    use crate::session::settings::review::ReviewResponse;
    use clacks_client_store::save::SaveOutcome;
    let basis = crate::review::tests::basis();
    let key = basis.record.key();
    for (outcome, draft_stale) in [
        (
            SettingsResponse::Failed("Could not save the resolution".into()),
            false,
        ),
        (
            SettingsResponse::Review(ReviewResponse::Resolved {
                key,
                outcome: Box::new(SaveOutcome::Stale {
                    current: Some(basis.record.clone()),
                }),
            }),
            true,
        ),
    ] {
        let (requests, mut received) = mpsc::channel(8);
        let context = egui::Context::default();
        let mut editor = comparison::ReviewEditor::new(basis.clone())?;
        editor.draft.select_all(ComparisonSource::Local)?;
        editor.draft.set_custom(
            FieldId::Qso(QsoField::Comment),
            FieldInput::Text {
                value: "My combined comment".into(),
            },
        );
        let chosen = editor
            .draft
            .build(&Default::default())
            .expect("the draft should build because every field is selected and the custom comment is valid");
        let mut view = ReviewUi {
            open: true,
            selected: Some(key),
            submit: Some(SaveRequest {
                expected: Box::new(basis.record.clone()),
                shown: Box::new(basis.review.clone()),
                chosen,
            }),
            drafts: [(key, editor)].into(),
            ..Default::default()
        };
        context
            .run_ui(egui::RawInput::default(), |ui| {
                view.show_window(ui.ctx(), &requests)
            })
            .textures_delta
            .clear();
        let request = received.try_recv()?;
        assert!(matches!(
            request.action,
            SettingsAction::Review(ReviewAction::Resolve { .. })
        ));
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        let mut output = context.run_ui(input, |ui| view.show_window(ui.ctx(), &requests));
        output.textures_delta.clear();
        assert!(view.open);
        assert!(output.viewport_output.values().any(|viewport| {
            viewport
                .commands
                .contains(&egui::ViewportCommand::CancelClose)
        }));

        assert!(request.reply.send(outcome).is_ok());
        assert!(view.poll());
        assert!(view.open);
        assert!(view.saving.is_none());
        let draft = &view
            .drafts
            .get(&key)
            .expect("the save did not commit, so the submitted draft should remain")
            .draft;
        assert_eq!(draft.is_stale(), draft_stale);
        assert!(
            matches!(draft.input(FieldId::Qso(QsoField::Comment))?, FieldInput::Text { value } if value == "My combined comment")
        );
        assert_eq!(draft.build(&Default::default()).is_err(), draft_stale);
    }
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
    let editor = comparison::ReviewEditor::new(basis)?;
    let mut settings = fixtures::snapshot();
    let input = editor
        .draft
        .source_input(FieldId::Qso(QsoField::Tags), &ComparisonSource::Local)?;
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
    ui.get_by_label("Use all local").click();
    ui.run();
    assert_eq!(
        ui.state()
            .editor
            .draft
            .selected_source(FieldId::Qso(QsoField::Comment)),
        Some(&ComparisonSource::Local)
    );
    let replacement =
        clacks_model::tag::LogbookTag::new(clacks_model::ids::TagId::new(), "Replacement".into());
    ui.get_by_label("Search logbooks").click();
    ui.run();
    ui.get_by_label("Search logbooks").type_text("Replacement");
    ui.run();
    ui.state_mut()
        .settings
        .logbooks
        .tags
        .push(fixtures::row(RecordValue::Tag(Box::new(
            replacement.clone(),
        ))));
    ui.run();
    assert!(ui.query_by_label("Replacement").is_some());
    assert!(ui.query_by_label("Logbook 0").is_none());
    assert_eq!(ui.state().editor.draft.membership(), Some(&tags));
    ui.get_by_label("Search logbooks")
        .set_text_value_accesskit("");
    ui.run();
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

#[test]
fn blocked_deletion_opens_its_referrer_and_retries_the_exact_target() -> anyhow::Result<()> {
    use crate::ui::tests::fixtures;
    use clacks_sync::client::ReviewReason;
    let tag = clacks_model::tag::LogbookTag::new(Default::default(), "Main".into());
    let mut basis = crate::review::tests::saved_basis(
        RecordValue::Tag(Box::new(tag.clone())),
        RecordValue::Tag(Box::new(tag)),
    );
    basis.review.local = RecordState::Deleted;
    let mut fields = basis.record.clone().into_fields();
    fields.pending.as_mut().unwrap().working = RecordState::Deleted;
    basis.record = fields.try_into()?;
    let referrer = fixtures::row(RecordValue::Qso(Box::new(fixtures::contact())));
    basis.review.reason = ReviewReason::Referenced {
        referrers: vec![referrer.key()],
        total: 1,
    };
    let key = basis.record.key();
    let view = ReviewUi {
        open: true,
        selected: Some(key),
        detail: Some(ReviewRow {
            record: basis.record.clone(),
            review: Some(basis.review.clone()),
            authorization_hold: None,
        }),
        references: Some(references::References {
            key,
            page: clacks_client_store::read::review::references::ReferencePage {
                rows: vec![ReviewRow {
                    record: referrer.clone(),
                    review: None,
                    authorization_hold: None,
                }],
                total: 1,
                next: None,
            },
        }),
        ..Default::default()
    };
    let (requests, mut received) = mpsc::channel(8);
    let mut ui = Harness::builder()
        .with_size(egui::vec2(900.0, 650.0))
        .build_ui_state(|ui, view| view.show_detail(ui, &requests), view);
    ui.get_by_label("Edit referring record").click();
    ui.step();
    assert_eq!(ui.state_mut().repair.take(), Some(referrer));
    ui.get_by_label("Refresh references").click();
    ui.step();
    assert_eq!(ui.state_mut().read_references.take(), Some((key, None)));
    ui.get_by_label("Retry deletion").click();
    ui.step();
    let request = received.try_recv()?;
    let SettingsAction::Review(ReviewAction::RetryDeletion { expected, shown }) = request.action
    else {
        panic!("retry")
    };
    assert_eq!(*expected, basis.record);
    assert_eq!(*shown, basis.review);
    Ok(())
}

#[test]
fn navigation_retains_drafts_and_late_reads_do_not_replace_the_selected_record()
-> anyhow::Result<()> {
    use crate::review::{
        fields::{FieldId, QsoField, TagField},
        input::FieldInput,
    };
    use crate::session::settings::review::ReviewResponse;
    let first = crate::review::tests::basis();
    let tag = clacks_model::tag::LogbookTag::new(Default::default(), "Main".into());
    let second = crate::review::tests::saved_basis(
        RecordValue::Tag(Box::new(tag.clone())),
        RecordValue::Tag(Box::new(tag)),
    );
    let first_key = first.record.key();
    let second_key = second.record.key();
    let first_row = ReviewRow {
        record: first.record.clone(),
        review: Some(first.review.clone()),
        authorization_hold: None,
    };
    let second_row = ReviewRow {
        record: second.record.clone(),
        review: Some(second.review.clone()),
        authorization_hold: None,
    };
    let first_caption = caption(&first.record, &Default::default());
    let second_caption = caption(&second.record, &Default::default());
    let mut first_editor = comparison::ReviewEditor::new(first)?;
    first_editor.draft.set_custom(
        FieldId::Qso(QsoField::Comment),
        FieldInput::Text {
            value: "Unfinished contact correction".into(),
        },
    );
    let mut second_editor = comparison::ReviewEditor::new(second)?;
    second_editor.draft.set_custom(
        FieldId::Tag(TagField::Name),
        FieldInput::Text {
            value: "Unfinished name correction".into(),
        },
    );
    let view = ReviewUi {
        open: true,
        selected: Some(first_key),
        conflicts: Section {
            rows: vec![first_row.clone(), second_row],
            next: None,
        },
        drafts: [(first_key, first_editor), (second_key, second_editor)].into(),
        ..Default::default()
    };
    let (requests, mut received) = mpsc::channel(8);
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1100.0, 700.0))
        .build_ui_state(
            |ui, view: &mut ReviewUi| {
                view.poll();
                view.show_contents(ui, &requests);
            },
            view,
        );
    ui.get_by_label(&second_caption).click();
    ui.step();
    assert_eq!(ui.state().selected, Some(second_key));
    assert!(
        ui.state_mut()
            .request(ReviewAction::ReadRecord { key: first_key }, &requests)
    );
    received
        .try_recv()?
        .reply
        .send(SettingsResponse::Review(ReviewResponse::Record {
            key: first_key,
            row: Some(Box::new(first_row)),
            settings: Box::new(crate::ui::tests::fixtures::snapshot()),
        }))
        .unwrap();
    ui.step();
    assert_eq!(ui.state().selected, Some(second_key));
    assert!(ui.state().detail.is_none());
    ui.get_by_label(&first_caption).click();
    ui.step();
    assert_eq!(ui.state().selected, Some(first_key));
    assert!(
        matches!(ui.state().drafts.get(&first_key).unwrap().draft.input(FieldId::Qso(QsoField::Comment))?, FieldInput::Text { value } if value == "Unfinished contact correction")
    );
    assert!(
        matches!(ui.state().drafts.get(&second_key).unwrap().draft.input(FieldId::Tag(TagField::Name))?, FieldInput::Text { value } if value == "Unfinished name correction")
    );
    Ok(())
}

#[test]
fn pending_values_are_inspectable_without_resolution_controls() -> anyhow::Result<()> {
    use crate::ui::tests::fixtures;
    let equipment: clacks_model::profile::EquipmentConfiguration =
        clacks_model::profile::EquipmentConfigurationFields {
            id: Default::default(),
            name: "Portable rig".into(),
            values: clacks_model::equipment::EquipmentValuesFields {
                notes: Some("New feed line".into()),
                ..Default::default()
            },
        }
        .try_into()?;
    let mut previous = equipment.clone().into_fields();
    previous.values.notes = Some("Original cable".into());
    let basis = crate::review::tests::saved_basis(
        RecordValue::Equipment(Box::new(equipment.clone())),
        RecordValue::Equipment(Box::new(previous.try_into()?)),
    );
    let key = basis.record.key();
    let row = ReviewRow {
        record: basis.record,
        review: None,
        authorization_hold: None,
    };
    let (requests, _received) = mpsc::channel(1);
    let view = ReviewUi {
        selected: Some(key),
        detail: Some(row.clone()),
        settings: Some(Box::new(fixtures::snapshot())),
        ..Default::default()
    };
    let ui = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(
            |ui, view: &mut ReviewUi| view.show_detail(ui, &requests),
            view,
        );
    assert!(ui.query_by_label("New feed line").is_some());
    assert!(ui.query_by_label("Save resolution").is_none());
    Ok(())
}

#[test]
fn settled_detail_does_not_claim_pending_synchronization() -> anyhow::Result<()> {
    use crate::session::settings::review::ReviewResponse;
    use crate::ui::tests::fixtures;
    let basis = crate::review::tests::basis();
    let key = basis.record.key();
    let mut fields = basis.record.into_fields();
    fields.pending = None;
    let accepted = ReviewRow {
        record: fields.try_into()?,
        review: None,
        authorization_hold: None,
    };
    let (requests, _received) = mpsc::channel(1);
    let mut view = ReviewUi {
        selected: Some(key),
        ..Default::default()
    };
    let (reply, response) = oneshot::channel();
    view.reply = Some(response);
    reply
        .send(SettingsResponse::Review(ReviewResponse::Record {
            key,
            row: Some(Box::new(accepted)),
            settings: Box::new(fixtures::snapshot()),
        }))
        .unwrap();
    view.poll();
    let ui = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(
            |ui, view: &mut ReviewUi| view.show_detail(ui, &requests),
            view,
        );
    assert!(ui.query_by_label("Saved in the shared log.").is_some());
    assert!(
        ui.query_by_label("Saved locally; pending synchronization.")
            .is_none()
    );
    Ok(())
}
