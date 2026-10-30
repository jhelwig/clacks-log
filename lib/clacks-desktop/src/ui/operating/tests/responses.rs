//! Acknowledgments and changing sources preserve the originating form's values.

use super::*;
use clacks_client_store::operating::OperatingSaveOutcome;

#[test]
fn failed_post_apply_refresh_uses_the_acknowledged_state_without_resubmitting() {
    let mut ui = mount();
    ui.get_by_label("Operating settings…").click();
    ui.run();
    replace(&mut ui, "Station callsign", "W7ACK");
    ui.get_by_label("Apply").click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    let SettingsAction::Apply { next, .. } = request.action else {
        panic!()
    };
    request
        .reply
        .send(SettingsResponse::RefreshFailed {
            outcome: Box::new(SettingsResponse::Apply(OperatingSaveOutcome::Saved(next))),
            message: "read unavailable".into(),
        })
        .unwrap();
    ui.run();
    ui.get_by_label("Operating settings…").click();
    ui.run();
    let Screen::Operating(editor) = &ui.state().view.stack.last().unwrap().screen else {
        panic!()
    };
    assert_eq!(editor.station.callsign, "W7ACK");
    assert_eq!(
        editor
            .expected
            .current
            .as_ref()
            .unwrap()
            .station
            .callsign
            .as_str(),
        "W7ACK"
    );
    assert!(ui.state_mut().requests.try_recv().is_err());
}

#[test]
fn deleted_source_clears_selection_while_retaining_pending_values() {
    let mut ui = mount();
    ui.get_by_label("Operating settings…").click();
    ui.run();
    ui.get_by_label("Operator / power").click();
    ui.run();
    replace(&mut ui, "Transmit power", "25");
    let mut updated = ui.state().snapshot.as_ref().clone();
    let profile = updated.profiles.first_mut().unwrap();
    let mut fields = profile.clone().into_fields();
    fields.pending.as_mut().unwrap().working = RecordState::Deleted;
    *profile = fields.try_into().unwrap();
    updated.operating.selected_profile = None;
    ui.state_mut().snapshot = Arc::new(updated);
    ui.run();
    let Screen::Operating(editor) = &ui.state().view.stack.last().unwrap().screen else {
        panic!()
    };
    assert!(editor.selected_profile.is_none());
    assert_eq!(editor.power_watts, "25");
    assert_eq!(editor.station.callsign, "W7HEX");
    assert!(ui.state_mut().requests.try_recv().is_err());
}
