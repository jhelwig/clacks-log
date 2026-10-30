//! Settings controls submit native requests without starting unrelated services.

mod quick;
mod responses;

use super::*;
use crate::ui::tests::fixtures;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

struct Host {
    view: SettingsUi,
    snapshot: Arc<SettingsSnapshot>,
    sender: mpsc::Sender<SettingsRequest>,
    requests: mpsc::Receiver<SettingsRequest>,
}

fn mount() -> Harness<'static, Host> {
    let (sender, requests) = mpsc::channel(8);
    Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1100.0, 1200.0))
        .build_ui_state(
            |ui, host| {
                host.view.management(ui, &host.sender);
                host.view.show(ui, &host.snapshot, &host.sender);
            },
            Host {
                view: Default::default(),
                snapshot: Arc::new(fixtures::snapshot()),
                sender,
                requests,
            },
        )
}

fn replace(ui: &mut Harness<'_, Host>, label: &str, value: &str) {
    ui.get_by_label(label).focus();
    ui.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    ui.get_by_label(label).type_text(value);
    ui.run();
}

#[test]
fn validation_blocks_apply_and_correction_submits_only_the_edited_scope() {
    let mut ui = mount();
    let before = ui.state().snapshot.operating.clone();
    ui.get_by_label("Operating settings…").click();
    ui.run();
    ui.get_by_label("Operator / power").click();
    ui.run();
    replace(&mut ui, "Transmit power", "0");
    assert!(ui.get_by_label("Apply").accesskit_node().is_disabled());
    ui.get_by_label("Apply").click();
    ui.step();
    assert!(ui.state_mut().requests.try_recv().is_err());

    replace(&mut ui, "Transmit power", ".5");
    assert!(!ui.get_by_label("Apply").accesskit_node().is_disabled());
    ui.get_by_label("Apply").click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    let SettingsAction::Apply { expected, next } = request.action else {
        panic!("Apply request")
    };
    assert_eq!(expected, before);
    assert_eq!(next.selected_profile, before.selected_profile);
    assert_eq!(next.selected_equipment, before.selected_equipment);
    assert_eq!(
        next.current.as_ref().unwrap().transmit_power.unwrap().get(),
        500
    );
    assert_eq!(ui.state().snapshot.operating, before);
}

#[test]
fn cancel_discards_operating_edits_and_restores_the_opener() {
    let mut ui = mount();
    ui.get_by_label("Operating settings…").click();
    ui.run();
    replace(&mut ui, "Station callsign", "W7OTHER");
    ui.get_by_label("Cancel operating changes").click();
    ui.run();
    assert!(ui.state_mut().requests.try_recv().is_err());
    assert!(ui.get_by_label("Operating settings…").is_focused());
    ui.get_by_label("Operating settings…").click();
    ui.run();
    let Screen::Operating(editor) = &ui.state().view.stack.last().unwrap().screen else {
        panic!()
    };
    assert_eq!(editor.station.callsign, "W7HEX");
}

#[test]
fn saving_a_profile_uses_pending_station_values_without_applying_them() {
    let mut ui = mount();
    ui.get_by_label("Operating settings…").click();
    ui.run();
    replace(&mut ui, "Station callsign", "W7PORTABLE");
    ui.get_by_label("Save station values as profile…").click();
    ui.run();
    replace(&mut ui, "Profile name", "Portable");
    ui.get_by_label("Save profile").click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    let SettingsAction::SaveProfile { next, .. } = request.action else {
        panic!("profile save")
    };
    assert_eq!(next.station().callsign.as_str(), "W7PORTABLE");
    assert_eq!(next.name(), "Portable");
    assert_eq!(
        ui.state()
            .snapshot
            .operating
            .current
            .as_ref()
            .unwrap()
            .station
            .callsign
            .as_str(),
        "W7HEX"
    );
    assert!(ui.state_mut().requests.try_recv().is_err());
}

#[test]
fn escape_discards_only_the_nested_editor() {
    let mut ui = mount();
    ui.get_by_label("Operating settings…").click();
    ui.run();
    replace(&mut ui, "Station callsign", "W7PORTABLE");
    ui.get_by_label("Save station values as profile…").click();
    ui.run();
    ui.key_press(egui::Key::Escape);
    ui.run();
    let Screen::Operating(editor) = &ui.state().view.stack.last().unwrap().screen else {
        panic!()
    };
    assert_eq!(editor.station.callsign, "W7PORTABLE");
    assert!(
        ui.get_by_label("Save station values as profile…")
            .is_focused()
    );
    assert!(ui.state_mut().requests.try_recv().is_err());
}

#[test]
fn quick_profile_selection_stages_station_values_and_preserves_independent_equipment() {
    let mut ui = mount();
    let before = ui.state().snapshot.operating.clone();
    let mut portable = before.selected_profile.clone().unwrap();
    {
        let mut fields = portable.clone().into_fields();
        fields.id = Default::default();
        portable = fields.try_into().expect("valid saved fixture edit");
    }
    {
        let mut fields = portable.clone().into_fields();
        fields.name = "Portable".into();
        portable = fields.try_into().expect("valid saved fixture edit");
    }
    {
        let mut fields = portable.clone().into_fields();
        fields.station.callsign = clacks_model::scalar::RecordedCallsign::new("W7HEX/P").unwrap();
        portable = fields.try_into().expect("valid saved fixture edit");
    }
    {
        let mut fields = portable.clone().into_fields();
        fields.default_equipment = None;
        portable = fields.try_into().expect("valid saved fixture edit");
    }
    Arc::make_mut(&mut ui.state_mut().snapshot)
        .profiles
        .push(fixtures::row(RecordValue::Profile(Box::new(
            portable.clone(),
        ))));
    ui.run();
    ui.get_by_label("Station").click();
    ui.run();
    ui.get_by_label(&format!("Load {}", profile_label(&portable)))
        .click();
    ui.run();
    assert_eq!(ui.state().snapshot.operating, before);
    assert!(ui.state_mut().requests.try_recv().is_err());
    ui.get_by_label("Apply").click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    let SettingsAction::Apply { expected, next } = request.action else {
        panic!()
    };
    let mut desired = before.clone();
    desired.current.as_mut().unwrap().station = portable.station().clone();
    desired.selected_profile = Some(portable);
    assert_eq!(expected, before);
    assert_eq!(next, desired);
}
