//! Inline edits retain later typing and honor explicit submission/cancellation.

use super::*;
use clacks_client_store::operating::OperatingSaveOutcome;

#[test]
fn inline_power_enter_submits_exact_watts_and_escape_discards_later_typing() {
    let mut ui = mount();
    replace(&mut ui, "Power", ".5");
    ui.key_press(egui::Key::Enter);
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    let SettingsAction::Apply { expected, next } = request.action else {
        panic!()
    };
    assert_eq!(expected, ui.state().snapshot.operating);
    assert_eq!(next.current().unwrap().transmit_power.unwrap().get(), 500);
    request
        .reply
        .send(SettingsResponse::Apply(OperatingSaveOutcome::Saved(
            next.clone(),
        )))
        .unwrap();
    ui.run();

    replace(&mut ui, "Power", "25");
    ui.key_press(egui::Key::Escape);
    ui.run();
    assert_eq!(ui.state().view.quick.power_text, "0.5");
    assert!(ui.state_mut().requests.try_recv().is_err());
}

#[test]
fn acknowledgment_does_not_overwrite_later_typing_or_repeat_a_failed_request() {
    let mut ui = mount();
    replace(&mut ui, "Power", "25");
    ui.key_press(egui::Key::Enter);
    ui.step();
    let first = ui.state_mut().requests.try_recv().unwrap();
    ui.get_by_label("Power").focus();
    ui.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    ui.get_by_label("Power").type_text("30");
    ui.step();
    ui.key_press(egui::Key::Tab);
    ui.step();
    assert!(ui.state_mut().requests.try_recv().is_err());
    let SettingsAction::Apply { next, .. } = first.action else {
        panic!()
    };
    first
        .reply
        .send(SettingsResponse::Apply(OperatingSaveOutcome::Saved(next)))
        .unwrap();
    ui.step();
    let second = ui.state_mut().requests.try_recv().unwrap();
    let SettingsAction::Apply { expected, next } = second.action else {
        panic!()
    };
    assert_eq!(
        expected.current().unwrap().transmit_power.unwrap().get(),
        25_000
    );
    assert_eq!(
        next.current().unwrap().transmit_power.unwrap().get(),
        30_000
    );
    second
        .reply
        .send(SettingsResponse::Failed(
            "Acknowledgment unavailable".into(),
        ))
        .unwrap();
    ui.run();
    assert_eq!(ui.state().view.quick.power_text, "30");
    assert!(ui.state_mut().requests.try_recv().is_err());
    assert!(ui.query_by_label("Acknowledgment unavailable").is_some());
}
