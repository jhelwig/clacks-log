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
        snapshot: None,
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
fn directory_pending_save_routes_result_by_committed_outcome() {
    use crate::directory::lookup::{LookupState, LookupUpdate};
    use clacks_model::directory::values::{
        DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields, LookupKey,
    };
    let mut view = ready_entry();
    let owner = view.entry.id;
    let key =
        LookupKey::from_recorded(&clacks_model::scalar::RecordedCallsign::new("OK2CQR").unwrap());
    let snapshot = DirectorySnapshot::new(DirectorySnapshotFields {
        provider: DirectoryProvider::HamQth,
        key: key.clone(),
        lookup_time: clacks_model::scalar::UtcTimestamp::new(1, 0).unwrap(),
        lookup_id: clacks_model::ids::LookupId::new(),
        facts: DirectoryFacts {
            name: Some("Directory only".into()),
            ..Default::default()
        },
    })
    .unwrap();
    let (sender, mut requests) = mpsc::channel(4);
    view.save(&sender);
    let request = requests.try_recv().unwrap();
    view.lookup_update(&LookupUpdate {
        changed: false,
        qso: owner,
        key: Some(key.clone()),
        state: LookupState::Found(Box::new(snapshot.clone())),
    });
    assert_eq!(view.entry.form.dx_name, "Recorded name");
    let mut older = DirectorySnapshotFields::from(snapshot.clone());
    older.lookup_time = clacks_model::scalar::UtcTimestamp::new(0, 0).unwrap();
    older.lookup_id = clacks_model::ids::LookupId::new();
    older.facts.name = Some("Earlier completion".into());
    view.lookup_update(&LookupUpdate {
        changed: false,
        qso: owner,
        key: Some(key.clone()),
        state: LookupState::Found(Box::new(DirectorySnapshot::new(older).unwrap())),
    });
    assert_eq!(
        view.directory.snapshot(&view.entry.form.dx.callsign),
        Some(Box::new(snapshot.clone()))
    );
    request
        .reply
        .send(SettingsResponse::Failed("Actual save failed".into()))
        .unwrap();
    view.poll();
    assert_eq!(view.entry.id, owner);
    view.save(&sender);
    let request = requests.try_recv().unwrap();
    let SettingsAction::Contacts(ContactAction::Save {
        next,
        snapshot: attached,
        ..
    }) = request.action
    else {
        panic!("save");
    };
    assert_eq!(attached, Some(Box::new(snapshot.clone())));
    assert_eq!(next.id(), owner);
    request
        .reply
        .send(SettingsResponse::Contacts(
            contacts::ContactResponse::Saved(clacks_client_store::save::SaveOutcome::Unchanged(
                crate::ui::tests::fixtures::row(RecordValue::Qso(next)),
            )),
        ))
        .unwrap();
    view.poll();
    assert_ne!(view.entry.id, owner);
    view.lookup_update(&LookupUpdate {
        changed: false,
        qso: owner,
        key: Some(key),
        state: LookupState::Found(Box::new(snapshot)),
    });
    assert!(
        view.directory
            .snapshot(&view.entry.form.dx.callsign)
            .is_none()
    );
}

#[test]
fn directory_fallback_is_attributed_and_keyboard_save_keeps_recorded_name_empty() {
    use crate::directory::lookup::{LookupState, LookupUpdate};
    use clacks_model::directory::values::{
        DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields, LookupKey,
    };
    use egui_kittest::kittest::NodeT;
    let mut view = ready_entry();
    view.entry.form.dx_name.clear();
    let (sender, mut requests) = mpsc::channel(8);
    view.directory
        .observe(view.entry.id, &view.entry.form.dx.callsign, &sender);
    requests.try_recv().unwrap();
    let key =
        LookupKey::from_recorded(&clacks_model::scalar::RecordedCallsign::new("OK2CQR").unwrap());
    view.lookup_update(&LookupUpdate {
        qso: view.entry.id,
        key: Some(key.clone()),
        changed: false,
        state: LookupState::Found(Box::new(
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
        )),
    });
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
        snapshot: Some(snapshot),
        ..
    }) = request.action
    else {
        panic!("save with separate snapshot");
    };
    assert!(next.contacted_operator().is_none());
    assert!(next.contacted_station().location.is_none());
    assert_eq!(
        snapshot.facts().name.as_deref(),
        Some("Separate directory name")
    );
}
