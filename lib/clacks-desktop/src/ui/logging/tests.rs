use super::*;
use crate::session::settings::contacts;

fn ready_entry() -> LoggingUi {
    let mut view = LoggingUi::default();
    view.entry.form.own.callsign = "W7HEX".into();
    view.entry.form.dx.callsign = "OK2CQR".into();
    view.entry.form.dx_name = "Recorded name".into();
    view.entry.form.primary.band = "20m".into();
    view.entry.form.mode = "SSB".into();
    view.entry.form.power = "5".into();
    view
}

#[test]
fn clear_uses_fresh_entry_reset_without_save() {
    let (sender, mut requests) = mpsc::channel(8);
    let mut entry = ready_entry();
    entry.entry.form.dx.callsign = "invalid!".into();
    entry.entry.form.comment = "Discard me".into();
    entry.entry.form.sent = "59".into();
    entry.entry.form.received = "59".into();
    entry.entry.form.receive.frequency = "7.074".into();
    entry.entry.form.receive.band = "40m".into();
    entry.entry.form.mode_parent = "PHONE".into();
    entry.active = fixtures::snapshot().operating;
    let tag = clacks_model::ids::TagId::new();
    entry.view = SelectedView::Logbook(tag);
    let old_id = entry.entry.id;
    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(800.0, 800.0))
        .build_ui_state(
            move |ui, entry: &mut LoggingUi| entry.show(ui, &sender),
            entry,
        );
    ui.get_by_role_and_label(egui::Role::Button, "Clear")
        .click();
    ui.run();
    assert!(ui.state().entry.form.dx.callsign.is_empty());
    assert!(ui.state().entry.form.dx_name.is_empty());
    assert!(ui.state().entry.form.comment.is_empty());
    assert!(ui.state().entry.form.sent.is_empty());
    assert_eq!(ui.state().entry.form.primary.band, "20m");
    assert_eq!(ui.state().entry.form.mode, "SSB");
    assert_eq!(ui.state().entry.form.mode_parent, "PHONE");
    assert_eq!(ui.state().entry.form.receive.frequency, "7.074");
    assert_eq!(ui.state().entry.form.receive.band, "40m");
    assert_eq!(ui.state().entry.form.tags, [tag].into());
    assert_eq!(ui.state().entry.form.own.callsign, "W7HEX");
    assert!(ui.get_by_label("Callsign").is_focused());
    let clear = ui.get_by_role_and_label(egui::Role::Button, "Clear").rect();
    let save = ui
        .get_by_role_and_label(egui::Role::Button, "Save QSO")
        .rect();
    assert!(clear.right() < save.left());
    assert!(ui.state().entry.form.start.live && ui.state().entry.form.end.live);
    assert_ne!(ui.state().entry.id, old_id);
    assert!(requests.try_recv().is_err());
}

#[test]
fn clear_is_disabled_only_for_pending_save() {
    use egui_kittest::kittest::NodeT;
    let (sender, mut requests) = mpsc::channel(8);
    let mut entry = ready_entry();
    entry.save(&sender);
    let request = requests.try_recv().unwrap();
    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(800.0, 800.0))
        .build_ui_state(
            move |ui, entry: &mut LoggingUi| entry.show(ui, &sender),
            entry,
        );
    assert!(
        ui.get_by_role_and_label(egui::Role::Button, "Clear")
            .accesskit_node()
            .is_disabled()
    );
    request
        .reply
        .send(SettingsResponse::Failed("Not admitted".into()))
        .unwrap();
    ui.run();
    assert!(
        !ui.get_by_role_and_label(egui::Role::Button, "Clear")
            .accesskit_node()
            .is_disabled()
    );
    ui.get_by_role_and_label(egui::Role::Button, "Clear")
        .click();
    ui.run();
    assert!(ui.state().entry.form.dx.callsign.is_empty());
    assert!(ui.state().notice.is_none());
    assert!(requests.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn authoritative_repeats_and_equal_key_changes_keep_correct_debounce() {
    let mut entry = ready_entry();
    entry.entry.form.dx.position.grid = "CN87".into();
    entry.entry.form.sent = "-10".into();
    entry.entry.form.received = "-07".into();
    entry.entry.form.comment = "Keep this".into();
    let mut expected = entry.entry.form.clone();
    let now = tokio::time::Instant::now();
    entry.incoming_call(" w7hex ");
    expected.dx.callsign = "W7HEX".into();
    assert_eq!(
        entry
            .entry
            .form
            .build(entry.entry.id, &entry.definitions)
            .unwrap(),
        expected.build(entry.entry.id, &entry.definitions).unwrap()
    );
    assert_eq!(
        entry.directory.submission().unwrap().due,
        now + std::time::Duration::from_millis(500)
    );
    tokio::time::advance(std::time::Duration::from_millis(200)).await;
    entry.incoming_call("W7HEX");
    assert_eq!(
        entry.directory.submission().unwrap().due,
        now + std::time::Duration::from_millis(500)
    );
    entry.entry.form.dx.callsign = "K1LOCAL".into();
    entry
        .directory
        .observe("K1LOCAL", tokio::time::Instant::now());
    entry.incoming_call("W7HEX");
    assert_eq!(entry.contact_call(), "W7HEX");
    assert_eq!(
        entry.directory.submission().unwrap().due,
        now + std::time::Duration::from_millis(700)
    );
    entry.incoming_call("W7HEX/P");
    let demand = entry.directory.submission().unwrap();
    assert_eq!(demand.key.as_str(), "W7HEX");
    assert_eq!(demand.due, now + std::time::Duration::from_millis(700));
    entry.incoming_call("");
    assert_eq!(entry.contact_call(), "");
    assert!(entry.directory.submission().is_none());
}

#[test]
fn incoming_status_does_not_rewrite_pending_submission() {
    for success in [false, true] {
        let mut entry = ready_entry();
        let (sender, mut requests) = mpsc::channel(8);
        entry.directory.observe(
            entry.contact_call().to_owned().as_str(),
            tokio::time::Instant::now(),
        );
        entry.save(&sender);
        let submitted = entry.submitted.clone().unwrap();
        let request = requests.try_recv().unwrap();
        let SettingsAction::Contacts(ContactAction::Save {
            next, directory, ..
        }) = request.action
        else {
            panic!()
        };
        entry.incoming_call("K1LATER");
        assert_eq!(entry.contact_call(), "K1LATER");
        assert_eq!(entry.submitted.as_ref(), Some(&submitted));
        assert_eq!(*next, submitted);
        assert_eq!(directory.unwrap().key.as_str(), "OK2CQR");
        entry.clear();
        assert_eq!(
            entry.contact_call(),
            "K1LATER",
            "pending saves disable Clear"
        );
        let response = if success {
            SettingsResponse::Contacts(contacts::ContactResponse::Saved(
                clacks_client_store::save::SaveOutcome::Saved(fixtures::row(RecordValue::Qso(
                    next,
                ))),
            ))
        } else {
            SettingsResponse::Failed("Not admitted".into())
        };
        request.reply.send(response).unwrap();
        entry.poll();
        if success {
            assert!(entry.contact_call().is_empty());
            assert!(entry.entry.form.dx_name.is_empty());
        } else {
            assert_eq!(entry.contact_call(), "K1LATER");
            assert_eq!(entry.entry.form.dx_name, "Recorded name");
            assert!(!entry.entry.form.start.live && !entry.entry.form.end.live);
        }
    }
}

#[tokio::test]
async fn clear_drops_display_interest_without_cancelling_shared_lookup() -> anyhow::Result<()> {
    use crate::directory::{
        lookup::tests::support::{Service, delayed, key, login, requests},
        test_support::fixture,
    };
    let (held, release) = delayed();
    let provider = fixture(vec![login(), held]).await?;
    let service = Service::new(provider.endpoint.clone()).await?;
    let context = egui::Context::default();
    let mut entry = ready_entry();
    entry.incoming_call("OK2CQR/P");
    let due = entry.directory.submission().unwrap().due;
    entry
        .directory
        .advance(&context, Some(&service.client), due);
    requests(&provider, 2).await;
    let shared = service.client.request(key("OK2CQR"));
    entry.clear();
    release
        .send(include_str!("../../directory/providers/fixtures/qrz.xml"))
        .unwrap();
    shared.wait().await;
    entry.advance_lookup(&context, Some(&service.client));
    assert!(entry.contact_call().is_empty());
    assert!(entry.directory.submission().is_none());
    assert!(
        entry
            .directory
            .fallback(&entry.entry.form, &entry.definitions)
            .name
            .is_none()
    );
    entry.incoming_call("OK2CQR");
    let due = entry.directory.submission().unwrap().due;
    entry
        .directory
        .advance(&context, Some(&service.client), due);
    assert_eq!(provider.lookup_count(), 1);
    service.close().await
}

#[test]
fn pending_submission_preserves_its_facts_and_resets_from_current_defaults() {
    let mut view = ready_entry();
    view.entry.form.start = crate::logging::time::ContactTimestamp::recorded(Some(
        clacks_model::scalar::UtcSecond::from_unix_seconds(1),
    ));
    view.entry.form.end = crate::logging::time::ContactTimestamp::recorded(None);
    let mut expected = fixtures::contact();
    crate::test_support::edit(&mut expected, |expected| {
        expected.id = view.entry.id;
        Ok(())
    });
    crate::test_support::edit(&mut expected, |expected| {
        expected.own_context.transmit_power =
            Some(clacks_model::scalar::Milliwatts::new(5000).unwrap());
        Ok(())
    });
    crate::test_support::edit(&mut expected, |expected| {
        expected.contacted_operator = Some(
            (clacks_model::contact::OperatorInformationFields {
                callsign: None,
                recorded_name: Some("Recorded name".into()),
            })
            .try_into()
            .expect("valid native fixture"),
        );
        Ok(())
    });
    let (sender, mut requests) = mpsc::channel(2);
    view.save(&sender);
    let submitted_id = expected.id();
    assert!(view.pending.is_some());
    view.view = SelectedView::Untagged;
    view.active = OperatingState::default();
    let request = requests.try_recv().unwrap();
    let SettingsAction::Contacts(action) = request.action else {
        panic!("wrong operation")
    };
    let contacts::ContactAction::Save {
        directory: None,
        expected: basis,
        next,
    } = action
    else {
        panic!()
    };
    assert!(basis.is_none());
    assert_eq!(*next, expected);
    assert_eq!(view.submitted.as_ref(), Some(&expected));
    let result = contacts::ContactResponse::Saved(clacks_client_store::save::SaveOutcome::Saved(
        crate::ui::tests::fixtures::row(clacks_sync::record::RecordValue::Qso(next)),
    ));

    request
        .reply
        .send(SettingsResponse::RefreshFailed {
            outcome: Box::new(SettingsResponse::Contacts(result)),
            message: "Refresh failed".into(),
        })
        .unwrap();
    view.poll();
    assert!(view.entry.form.dx.callsign.is_empty());
    assert!(view.entry.form.dx_name.is_empty());
    assert_eq!(view.entry.form.primary.band, "20m");
    assert!(view.entry.form.power.is_empty());
    assert!(view.entry.form.tags.is_empty());
    assert!(view.entry.form.start.live && view.entry.form.end.live);
    assert_ne!(view.entry.id, submitted_id);
    assert!(view.changed);
    assert_eq!(view.notice.as_deref(), Some("Refresh failed"));
    view.poll();
    assert_ne!(view.entry.id, submitted_id);
}

#[test]
fn failed_save_preserves_input_and_retry_identity() {
    let mut view = ready_entry();
    let (sender, mut requests) = mpsc::channel(1);
    view.save(&sender);
    let id = view.entry.id;
    let request = requests.try_recv().unwrap();
    request
        .reply
        .send(SettingsResponse::Failed("Workspace unavailable".into()))
        .unwrap();
    view.poll();
    assert_eq!(view.entry.form.dx.callsign, "OK2CQR");
    assert_eq!(view.entry.form.dx_name, "Recorded name");
    assert_eq!(view.entry.id, id);
    assert!(!view.entry.form.start.live);
    assert!(view.pending.is_none());
    assert!(!view.changed);
}

use crate::ui::tests::fixtures;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn entry_header_and_save_remain_visible_while_the_form_scrolls() {
    let (sender, mut requests) = mpsc::channel(8);
    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_context_setup(|ctx| crate::ui::theme::install(ctx, &Default::default()))
        .with_size(egui::vec2(550.0, 220.0))
        .build_ui_state(
            move |ui, entry: &mut LoggingUi| entry.show(ui, &sender),
            ready_entry(),
        );
    let header = ui.get_by_label("QSO entry").rect();
    let save = ui
        .get_by_role_and_label(egui::Role::Button, "Save QSO")
        .rect();
    assert!((header.center().y - save.center().y).abs() < 1.0);
    assert!(save.left() > header.right());
    assert!((save.right() - 534.0).abs() < 1.0);
    let divider_y = ui
        .output()
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::LineSegment {
                points: [left, right],
                ..
            } if (left.y - right.y).abs() < 0.5
                && (right.x - left.x).abs() > 500.0
                && left.y > save.bottom() =>
            {
                Some(left.y)
            }
            _ => None,
        })
        .min_by(f32::total_cmp)
        .expect("header divider");
    let top_padding = save.top() - 8.0;
    assert!((top_padding - (542.0 - save.right())).abs() < 0.5);
    assert!(
        (top_padding - (divider_y - save.bottom())).abs() < 0.5,
        "header padding must be symmetric: top={top_padding}, bottom={}",
        divider_y - save.bottom()
    );
    let call_top = ui
        .get_by_role_and_label(egui::Role::TextInput, "Callsign")
        .rect()
        .top();
    ui.get_by_label("Comment").scroll_to_me();
    ui.run();
    assert_eq!(ui.get_by_label("QSO entry").rect(), header);
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::Button, "Save QSO")
            .rect(),
        save
    );
    assert!(
        ui.get_by_role_and_label(egui::Role::TextInput, "Callsign")
            .rect()
            .top()
            < call_top
    );
    ui.get_by_role_and_label(egui::Role::Button, "Save QSO")
        .click();
    ui.run();
    assert!(
        std::iter::from_fn(|| requests.try_recv().ok()).any(|request| {
            matches!(
                request.action,
                SettingsAction::Contacts(ContactAction::Save { .. })
            )
        })
    );
}

#[test]
fn reports_align_and_timestamp_toggles_fill_the_input_height() {
    for (width, font_scale) in [(550.0, 1.0), (1100.0, 1.0), (550.0, 1.5)] {
        for edited in [false, true] {
            let mut form = ready_entry().entry.form;
            let baseline = form.clone();
            if edited {
                form.sent = "59".into();
                form.received = "59".into();
            }
            let mut ui = Harness::builder()
                .allow_missing_glyphs()
                .with_context_setup(move |context| {
                    crate::ui::theme::install(context, &Default::default());
                    context.all_styles_mut(|style| {
                        for font in style.text_styles.values_mut() {
                            font.size *= font_scale;
                        }
                    });
                })
                .with_size(egui::vec2(width, 700.0))
                .build_ui_state(
                    move |ui, form| {
                        let errors = if edited {
                            vec![crate::operating::FieldError {
                                field: "Sent".into(),
                                message: "A long validation message must remain within its report column.".into(),
                            }]
                        } else { vec![] };
                        entry::show(ui, form, &baseline, &errors, false, true, &Default::default());
                    },
                    form,
                );
            for locked in [false, true] {
                let sent = ui
                    .get_by_role_and_label(egui::Role::TextInput, "Sent")
                    .rect();
                let received = ui
                    .get_by_role_and_label(egui::Role::TextInput, "Received")
                    .rect();
                assert!(
                    (sent.top() - received.top()).abs() < 1.0,
                    "{sent:?} {received:?}"
                );
                assert!(sent.right() <= received.left());
                let toggles: Vec<_> = ui
                    .get_all_by_label(if locked { "Locked" } else { "Live" })
                    .collect();
                assert_eq!(toggles.len(), 2);
                for (label, toggle) in ["Start · UTC", "End · UTC"].into_iter().zip(toggles) {
                    let input = ui
                        .get_by_role_and_label(egui::Role::TextInput, label)
                        .rect();
                    let button = toggle.rect();
                    assert!(button.right() <= width - 8.0, "{width}: {button:?}");
                    assert!(
                        (input.top() - button.top()).abs() < 1.0,
                        "{input:?} {button:?}"
                    );
                    assert!(
                        (input.height() - button.height()).abs() < 1.0,
                        "{input:?} {button:?}"
                    );
                }
                if !locked {
                    ui.state_mut().start.lock();
                    ui.state_mut().end.lock();
                    ui.run();
                }
            }
        }
    }
}

#[test]
fn reflow_spacing_is_content_sized_at_different_pane_heights() {
    for font_scale in [1.0, 1.5] {
        let mut previous_positions: Option<Vec<f32>> = None;
        for height in [400.0, 700.0, 1100.0] {
            let mut form = ready_entry().entry.form;
            let baseline = form.clone();
            form.sent = "59".into();
            form.received = "59".into();
            let ui = Harness::builder()
                .allow_missing_glyphs()
                .with_context_setup(move |context| {
                    crate::ui::theme::install(context, &Default::default());
                    context.all_styles_mut(|style| {
                        for font in style.text_styles.values_mut() {
                            font.size *= font_scale;
                        }
                    });
                })
                .with_size(egui::vec2(550.0, height))
                .build_ui_state(
                    move |ui, form| {
                        entry::show(ui, form, &baseline, &[], false, true, &Default::default());
                    },
                    form,
                );
            let sent = ui
                .get_by_role_and_label(egui::Role::TextInput, "Sent")
                .rect();
            let start = ui
                .get_by_role_and_label(egui::Role::TextInput, "Start · UTC")
                .rect();
            let end = ui
                .get_by_role_and_label(egui::Role::TextInput, "End · UTC")
                .rect();
            let comment = ui.get_by_label("Comment").rect();
            assert!(
                (0.0..80.0).contains(&(start.top() - sent.bottom())),
                "reflow must follow the reports without filling the pane: height={height}, {sent:?}, {start:?}"
            );
            if font_scale == 1.0 {
                assert!((start.top() - end.top()).abs() < 1.0);
            } else {
                assert!((0.0..80.0).contains(&(end.top() - start.bottom())));
            }
            assert!((0.0..80.0).contains(&(comment.top() - end.bottom())));
            let positions = vec![sent.top(), start.top(), end.top(), comment.top()];
            if let Some(previous) = &previous_positions {
                for (before, after) in previous.iter().zip(&positions) {
                    assert!(
                        (before - after).abs() < 1.0,
                        "pane height must not change form spacing: {previous:?} -> {positions:?}"
                    );
                }
            }
            previous_positions = Some(positions);
        }
    }
}

#[test]
fn single_line_enter_saves_but_multiline_enter_edits_the_comment() {
    struct Entry {
        form: crate::logging::form::ContactForm,
        save: bool,
    }
    let entry = ready_entry();
    let baseline = entry.entry.form.clone();
    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1100.0, 500.0))
        .build_ui_state(
            move |ui, state: &mut Entry| {
                state.save |= entry::show(
                    ui,
                    &mut state.form,
                    &baseline,
                    &[],
                    false,
                    true,
                    &Default::default(),
                )
                .save;
            },
            Entry {
                form: entry.entry.form,
                save: false,
            },
        );
    ui.get_by_label("Callsign").focus();
    ui.key_press(egui::Key::Enter);
    ui.step();
    assert!(ui.state().save);
    ui.state_mut().save = false;
    ui.get_by_label("Comment").focus();
    ui.key_press(egui::Key::Enter);
    ui.step();
    assert!(!ui.state().save);
    assert!(ui.state().form.comment.contains('\n'));
}

#[test]
fn station_inputs_in_entry_and_saved_editor_have_independent_widget_identity() {
    struct Forms {
        entry: crate::logging::form::ContactForm,
        saved: crate::logging::form::ContactForm,
    }
    let form = ready_entry().entry.form;
    let baseline = form.clone();
    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1400.0, 2200.0))
        .build_ui_state(
            move |ui, forms: &mut Forms| {
                ui.columns(2, |columns| {
                    let mut columns = columns.iter_mut();
                    for (column, form, owner) in [
                        (columns.next().unwrap(), &mut forms.entry, "entry.details"),
                        (columns.next().unwrap(), &mut forms.saved, "saved.contact"),
                    ] {
                        details::show(
                            column,
                            form,
                            &baseline,
                            &DefinitionSet::default(),
                            &[],
                            crate::ui::logging::details::DetailPresentation {
                                own: true,
                                owner,
                                fallback: &Default::default(),
                            },
                        );
                    }
                });
            },
            Forms {
                entry: form.clone(),
                saved: form,
            },
        );
    ui.get_all_by_label("Station callsign")
        .nth(1)
        .unwrap()
        .focus();
    ui.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    ui.get_all_by_label("Station callsign")
        .nth(1)
        .unwrap()
        .type_text("K1EDITOR");
    ui.run();
    assert_eq!(ui.state().saved.own.callsign, "K1EDITOR");
    assert_eq!(ui.state().entry.own.callsign, "W7HEX");
}

#[test]
fn detail_fields_follow_the_available_pane_width() {
    for width in [420.0, 1000.0] {
        let (sender, _requests) = mpsc::channel(2);
        let view = ready_entry();

        let mut ui = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(width, 2400.0))
            .build_ui_state(
                move |ui, view: &mut LoggingUi| view.details(ui, &sender, true),
                view,
            );
        ui.run();
        for heading in [
            "Station identity",
            "Operating position",
            "Geographic details",
        ] {
            let heading = ui.get_by_label(heading).rect();
            assert!(
                !ui.output().shapes.iter().any(|shape| matches!(
                    &shape.shape,
                    egui::Shape::Rect(rect)
                        if rect.stroke.width > 0.0 && rect.rect.contains_rect(heading)
                )),
                "form sections must not be inset cards"
            );
        }
        for label in [
            "Station callsign",
            "Latitude",
            "Longitude",
            "DXCC entity",
            "State / province",
            "County / subdivision",
        ] {
            assert!(
                ui.get_by_label(label).rect().width() >= width - 50.0,
                "{label} at {width}"
            );
        }
    }
}

#[test]
fn applied_defaults_are_the_edit_baseline_and_contact_corrections_survive_refresh() {
    let mut snapshot = fixtures::snapshot();
    let mut view = LoggingUi::default();
    view.refresh(&snapshot);
    let (sender, _requests) = mpsc::channel(2);
    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(900.0, 2400.0))
        .build_ui_state(
            move |ui, view: &mut LoggingUi| view.details(ui, &sender, true),
            view,
        );
    ui.run();
    assert!(ui.query_all_by_label("Not applied").next().is_none());
    ui.get_by_label("Station callsign").focus();
    ui.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    ui.get_by_label("Station callsign")
        .type_text("W7CORRECTION");
    ui.run();
    ui.state_mut().refresh(&snapshot);
    assert_eq!(ui.state().entry.form.own.callsign, "W7CORRECTION");
    let mut current = snapshot.operating.current().cloned().unwrap();
    let mut equipment = current.equipment.take().unwrap().into_fields();
    equipment.equipment_description = Some("KX3".into());
    current.equipment = Some(equipment.try_into().unwrap());
    snapshot.operating.adjust(current);
    ui.state_mut().refresh(&snapshot);
    ui.run();
    assert_eq!(ui.state().entry.form.equipment.description, "KX3");
    assert_eq!(ui.state().entry.form.own.callsign, "W7CORRECTION");
    assert_eq!(ui.query_all_by_label("Not applied").count(), 1);
}

#[test]
fn directory_fallback_is_attributed_and_keyboard_save_keeps_recorded_name_empty() {
    use clacks_model::directory::values::{
        DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields, LookupKey,
    };
    use egui_kittest::kittest::NodeT;
    let mut view = ready_entry();
    view.entry.form.dx_name.clear();
    let (sender, mut requests) = mpsc::channel(8);
    view.directory
        .observe(&view.entry.form.dx.callsign, tokio::time::Instant::now());
    let key =
        LookupKey::from_recorded(&clacks_model::scalar::RecordedCallsign::new("OK2CQR").unwrap());
    view.directory.retain(Some(Box::new(
        DirectorySnapshot::new(DirectorySnapshotFields {
            provider: DirectoryProvider::HamQth,
            key,
            lookup_time: clacks_model::scalar::UtcTimestamp::new(1, 0).unwrap(),
            lookup_id: clacks_model::ids::LookupId::new(),
            facts: DirectoryFacts {
                name: Some("Separate directory name".into()),
                locality: Some("Directory locality".into()),
                location: Some(
                    clacks_model::location::RecordedLocationFields {
                        position: Some(clacks_model::position::Position::GridArea(
                            clacks_model::position::Locator::parse("FN31").unwrap(),
                        )),
                        dxcc: Some("291".into()),
                        ..Default::default()
                    }
                    .try_into()
                    .unwrap(),
                ),
                ..Default::default()
            },
        })
        .unwrap(),
    )));
    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1400.0, 1600.0))
        .build_ui_state(
            move |ui, view: &mut LoggingUi| {
                ui.columns(2, |columns| {
                    let mut columns = columns.iter_mut();
                    view.show(columns.next().unwrap(), &sender);
                    view.details(columns.next().unwrap(), &sender, false);
                });
            },
            view,
        );
    assert!(
        ui.query_by_label("HamQTH name: Separate directory name")
            .is_none()
    );
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::TextInput, "Name")
            .accesskit_node()
            .data()
            .placeholder(),
        Some("Separate directory name")
    );
    ui.get_by_role_and_label(egui::Role::TextInput, "Name")
        .focus();
    ui.run();
    ui.get_by_role_and_label(egui::Role::TextInput, "Name")
        .type_text("   ");
    ui.key_press(egui::Key::Tab);
    ui.run();
    assert!(ui.state().entry.form.dx_name.is_empty());
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::TextInput, "Name")
            .accesskit_node()
            .data()
            .placeholder(),
        Some("Separate directory name")
    );
    assert!(ui.query_all_by_label("HamQTH").count() > 0);
    for (label, expected) in [
        ("DX grid", "FN31"),
        ("DXCC entity", "291"),
        ("Latitude", "41.500000"),
    ] {
        assert_eq!(
            ui.get_by_role_and_label(egui::Role::TextInput, label)
                .accesskit_node()
                .data()
                .placeholder(),
            Some(expected)
        );
    }
    assert!(
        ui.query_by_label("HamQTH locality: Directory locality")
            .is_some()
    );
    assert!(
        ui.query_by_label("Callbook data provided by HamQTH.com")
            .is_some()
    );
    ui.get_by_label("Add reference").click();
    ui.run();
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::TextInput, "DX grid")
            .accesskit_node()
            .data()
            .placeholder(),
        Some("FN31")
    );
    ui.get_by_label("Remove reference").click();
    ui.run();
    // A partial recorded location suppresses the entire directory location.
    ui.get_by_role_and_label(egui::Role::TextInput, "State / province")
        .focus();
    ui.run();
    ui.get_by_role_and_label(egui::Role::TextInput, "State / province")
        .type_text("CT");
    ui.run();
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::TextInput, "DX grid")
            .accesskit_node()
            .data()
            .placeholder(),
        Some("")
    );
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::TextInput, "DXCC entity")
            .accesskit_node()
            .data()
            .placeholder(),
        Some("")
    );
    ui.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    ui.key_press(egui::Key::Backspace);
    ui.run();
    ui.get_by_role_and_label(egui::Role::TextInput, "Name")
        .focus();
    ui.run();
    ui.get_by_role_and_label(egui::Role::TextInput, "Name")
        .type_text("My recorded name");
    ui.run();
    assert_eq!(ui.state().entry.form.dx_name, "My recorded name");
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::TextInput, "Name")
            .accesskit_node()
            .data()
            .placeholder(),
        Some("")
    );
    ui.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    ui.key_press(egui::Key::Backspace);
    ui.run();
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::TextInput, "Name")
            .accesskit_node()
            .data()
            .placeholder(),
        Some("Separate directory name")
    );
    ui.get_by_role_and_label(egui::Role::TextInput, "Name")
        .focus();
    ui.key_press(egui::Key::Tab);
    ui.run();
    assert!(ui.state().entry.form.dx_name.is_empty());
    ui.get_by_role_and_label(egui::Role::TextInput, "Name")
        .focus();
    ui.key_press(egui::Key::Enter);
    ui.run();
    let request = requests.try_recv().unwrap();
    let SettingsAction::Contacts(ContactAction::Save {
        next,
        directory: Some(demand),
        ..
    }) = request.action
    else {
        panic!("save with separate snapshot");
    };
    assert!(next.contacted_operator().is_none());
    assert!(next.contacted_station().location.is_none());
    let result = demand.handle.unwrap().ready().unwrap();
    let crate::directory::lookup::LookupState::Found(snapshot) = result.as_ref() else {
        panic!("directory facts")
    };
    assert_eq!(
        snapshot.facts().name.as_deref(),
        Some("Separate directory name")
    );
}

#[tokio::test]
async fn current_failed_entry_resumes_on_usable_configuration() -> anyhow::Result<()> {
    use crate::directory::{
        lookup::tests::support::Service,
        test_support::{fixture, response},
    };
    let provider = fixture(vec![
        response(
            200,
            "<QRZDatabase><Session><Key>fixture-key</Key></Session></QRZDatabase>",
        ),
        response(
            200,
            include_str!("../../directory/providers/fixtures/qrz.xml"),
        ),
    ])
    .await?;
    let service = Service::new(provider.endpoint.clone()).await?;
    service.available(false);
    let mut view = ready_entry();
    let context = egui::Context::default();
    let now = tokio::time::Instant::now();
    view.directory.observe(&view.entry.form.dx.callsign, now);
    view.directory.advance(
        &context,
        Some(&service.client),
        now + std::time::Duration::from_millis(500),
    );
    assert_eq!(provider.lookup_count(), 0);
    service.available(true);
    view.directory.advance(
        &context,
        Some(&service.client),
        now + std::time::Duration::from_millis(501),
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while provider.lookup_count() == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await?;
    service
        .client
        .request(clacks_model::directory::values::LookupKey::from_recorded(
            &clacks_model::scalar::RecordedCallsign::new("OK2CQR")?,
        ))
        .wait()
        .await;
    view.directory.advance(
        &context,
        Some(&service.client),
        now + std::time::Duration::from_secs(1),
    );
    assert_eq!(provider.lookup_count(), 1);
    service.available(true);
    view.directory.advance(
        &context,
        Some(&service.client),
        now + std::time::Duration::from_secs(2),
    );
    assert_eq!(
        provider.lookup_count(),
        1,
        "usable configuration does not refresh a successful result"
    );
    service.close().await
}
