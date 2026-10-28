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
    expected.id = view.entry.id;
    expected.own_context.transmit_power =
        Some(clacks_model::scalar::Milliwatts::new(5000).unwrap());
    expected.contacted_operator = Some(clacks_model::contact::OperatorInformation {
        callsign: None,
        recorded_name: Some("Recorded name".into()),
    });
    let (sender, mut requests) = mpsc::channel(2);
    view.save(&sender);
    let submitted_id = expected.id;
    assert!(view.pending.is_some());
    view.view = SelectedView::Untagged;
    view.active = OperatingState::default();
    let request = requests.try_recv().unwrap();
    let SettingsAction::Contacts(action) = request.action else {
        panic!("wrong operation")
    };
    let contacts::ContactAction::Save {
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
                state.save |= entry::show(ui, &mut state.form, &baseline, &[], false, true).save;
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
                            true,
                            owner,
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
    snapshot
        .operating
        .current
        .as_mut()
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .equipment_description = Some("KX3".into());
    snapshot.operating.selected_equipment = None;
    ui.state_mut().refresh(&snapshot);
    ui.run();
    assert_eq!(ui.state().entry.form.equipment.description, "KX3");
    assert_eq!(ui.state().entry.form.own.callsign, "W7CORRECTION");
    assert_eq!(ui.query_all_by_label("Not applied").count(), 1);
}
