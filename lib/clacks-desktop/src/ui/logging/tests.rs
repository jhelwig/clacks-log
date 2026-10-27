use super::*;
use crate::session::settings::contacts;
use clacks_client_store::store::{ClientStore, StoreSettings};

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

#[tokio::test]
async fn pending_submission_preserves_its_facts_and_resets_from_current_defaults()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("entry.sqlite"),
        Default::default(),
        StoreSettings::default(),
    )
    .await?;
    let mut view = ready_entry();
    let (sender, mut requests) = mpsc::channel(2);
    view.save(&sender);
    let submitted_id = view.submitted.as_ref().unwrap().id;
    assert!(view.pending.is_some());
    view.view = SelectedView::Untagged;
    view.active = OperatingState::default();
    let request = requests.recv().await.unwrap();
    let SettingsAction::Contacts(action) = request.action else {
        anyhow::bail!("wrong operation")
    };
    let result = contacts::execute(&store, action).await?;
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
    store.close().await?;
    Ok(())
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

#[test]
fn single_line_enter_saves_without_stealing_multiline_input() {
    use eframe::egui::{Event, Key, Modifiers};
    let context = egui::Context::default();
    let mut view = ready_entry();
    let baseline = view.entry.form.clone();
    let mut save = false;
    let input = |events| egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 600.0),
        )),
        events,
        ..Default::default()
    };
    let key = || Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let mut call = egui::Id::NULL;
    let mut comment = egui::Id::NULL;
    for _ in 0..2 {
        context
            .run_ui(input(vec![]), |ui| {
                call = ui.make_persistent_id("entry.call");
                comment = ui.make_persistent_id("entry.comment");
                entry::show(ui, &mut view.entry.form, &baseline, &[], true, true);
            })
            .drop_without_applying_deltas();
    }
    call = context
        .memory(|m| m.focused())
        .expect("quick callsign receives requested focus");
    context.memory_mut(|m| m.request_focus(call));
    context
        .run_ui(input(vec![key()]), |ui| {
            save = entry::show(ui, &mut view.entry.form, &baseline, &[], false, true).save;
        })
        .drop_without_applying_deltas();
    assert!(save, "single-line Enter should request Save QSO");
    context.memory_mut(|m| m.request_focus(comment));
    context
        .run_ui(input(vec![key()]), |ui| {
            save = entry::show(ui, &mut view.entry.form, &baseline, &[], false, true).save;
        })
        .drop_without_applying_deltas();
    assert!(!save, "multiline Enter must remain an edit");
}

#[test]
fn shared_station_widgets_keep_entry_and_saved_editor_input_independent() {
    use eframe::egui::{Event, Modifiers, PointerButton};
    let context = egui::Context::default();
    let mut entry = ready_entry().entry.form;
    let mut editor = ready_entry().entry.form;
    entry.own.callsign = "W7ENTRY".into();
    editor.own.callsign = "K1EDITOR".into();
    let entry_basis = entry.clone();
    let editor_basis = editor.clone();
    let mut frame = |events| {
        context.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 2000.0),
                )),
                ..Default::default()
            },
            |ui| {
                ui.columns(2, |columns| {
                    let mut columns = columns.iter_mut();
                    details::show(
                        columns.next().unwrap(),
                        &mut entry,
                        &entry_basis,
                        &DefinitionSet::default(),
                        &[],
                        true,
                        "entry.details",
                    );
                    details::show(
                        columns.next().unwrap(),
                        &mut editor,
                        &editor_basis,
                        &DefinitionSet::default(),
                        &[],
                        true,
                        "saved.contact",
                    );
                });
            },
        )
    };
    frame(vec![]).drop_without_applying_deltas();
    let mut output = frame(vec![]);
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == "K1EDITOR" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .expect("saved editor callsign is rendered");
    output.textures_delta.clear();
    frame(vec![
        Event::PointerMoved(position),
        Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
    ])
    .drop_without_applying_deltas();
    frame(vec![Event::PointerButton {
        pos: position,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }])
    .drop_without_applying_deltas();
    frame(vec![Event::Text("Z".into())]).drop_without_applying_deltas();
    assert_ne!(editor.own.callsign, "K1EDITOR");
    assert_eq!(entry.own.callsign, "W7ENTRY");
}

#[test]
fn detail_groups_and_fields_follow_the_available_pane_width() {
    use crate::ui::{preferences::AppearancePreferences, theme};

    for own in [false, true] {
        for width in [420.0, 800.0, 1200.0] {
            let context = egui::Context::default();
            theme::install(&context, &AppearancePreferences::default());
            let mut view = ready_entry();
            view.tab = if own {
                DetailTab::OurSettings
            } else {
                DetailTab::Contact
            };
            let (sender, _requests) = mpsc::channel(2);
            let mut available = egui::Rect::NOTHING;
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 3000.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default()
                        .frame(theme::section_frame(ui))
                        .show(ui, |ui| {
                            available = ui.available_rect_before_wrap();
                            view.details(ui, &sender);
                        });
                },
            );
            output.textures_delta.clear();
            for heading in [
                "Operating position",
                "Geographic details",
                "Participation / references",
            ] {
                let label = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) if text.galley.text() == heading => {
                            Some(text.visual_bounding_rect())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("missing {heading}"));
                let group = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Rect(rect)
                            if rect.stroke.width > 0.0 && rect.rect.contains_rect(label) =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .min_by(|a, b| a.area().total_cmp(&b.area()))
                    .unwrap();
                assert!(
                    group.width() >= available.width() - 20.0,
                    "{heading} must fill pane width: {group:?}, available {available:?}"
                );
            }
            let side = if own { "own" } else { "dx" };
            for field in ["latitude", "longitude", "dxcc", "state", "county"] {
                let id = egui::Id::unique(format!("entry.details.{side}.{field}"));
                let response = context.read_response(id).unwrap();
                assert!(
                    response.rect.width() >= available.width() - 32.0,
                    "{side}.{field} must fill its group: {:?}, available {available:?}",
                    response.rect
                );
            }
            output.drop_without_applying_deltas();
        }
    }
}

#[tokio::test]
async fn applied_profile_defaults_are_not_presented_as_unapplied_contact_edits()
-> anyhow::Result<()> {
    use crate::operating::{editor::OperatingEditor, station::StationForm};
    use clacks_model::{
        equipment::EquipmentValues,
        profile::{EquipmentConfiguration, SavedLocationProfile},
    };

    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("defaults.sqlite"),
        Default::default(),
        StoreSettings::default(),
    )
    .await?;
    let equipment = EquipmentConfiguration {
        id: Default::default(),
        name: "Rig A".into(),
        values: EquipmentValues {
            equipment_description: Some("IC-7300".into()),
            ..Default::default()
        },
    };
    let mut station = StationForm::empty();
    station.callsign = "W7HEX".into();
    station.edit_grid("CN85".into());
    let profile = SavedLocationProfile {
        id: Default::default(),
        name: "Home".into(),
        station: station.build(&DefinitionSet::default()).unwrap(),
        default_equipment: Some(equipment.id),
    };
    clacks_client_store::settings::save_equipment(&store, None, &equipment).await?;
    clacks_client_store::settings::save_profile(&store, None, &profile).await?;
    let mut editor = OperatingEditor::new(&OperatingState::default());
    editor.stage_profile(&profile, Some(&equipment))?;
    let applied = editor.build(&DefinitionSet::default()).unwrap();
    clacks_client_store::operating::save(&store, &editor.expected, &applied).await?;
    let snapshot = clacks_client_store::settings::snapshot(&store).await?;
    let (sender, _requests) = mpsc::channel(2);
    let mut view = LoggingUi {
        tab: DetailTab::OurSettings,
        ..Default::default()
    };
    view.refresh(&snapshot);
    let context = egui::Context::default();
    let markers = |view: &mut LoggingUi| {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 3000.0),
                )),
                ..Default::default()
            },
            |ui| view.details(ui, &sender),
        );
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter(|shape| {
                matches!(&shape.shape,
                    egui::epaint::Shape::Text(text) if text.galley.text() == "Not applied"
                )
            })
            .count()
    };
    assert_eq!(view.entry.form.own.callsign, "W7HEX");
    assert_eq!(view.entry.form.equipment.description, "IC-7300");
    assert_eq!(
        markers(&mut view),
        0,
        "applied defaults are the entry baseline"
    );

    let before = view.entry.form.clone();
    view.entry.form.own.callsign = "W7CORRECTION".into();
    view.entry.capture_corrections(&before);
    view.refresh(&snapshot);
    assert_eq!(
        markers(&mut view),
        1,
        "retain the explicit contact correction"
    );

    let mut next = snapshot.clone();
    next.operating.selected_equipment = None;
    next.operating
        .current
        .as_mut()
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .equipment_description = Some("KX3".into());
    view.refresh(&next);
    assert_eq!(view.entry.form.equipment.description, "KX3");
    assert_eq!(view.entry.form.own.callsign, "W7CORRECTION");
    assert_eq!(
        markers(&mut view),
        1,
        "following new defaults adds no edit markers"
    );
    store.close().await?;
    Ok(())
}
