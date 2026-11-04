//! Receiver intent tests catch incorrect active-field submission and dismissal.
use crate::{
    session::SessionCommand,
    ui::ws_suite::ReceiverUi,
    ws_suite::{ReceiverAction, ReceiverView},
};
use clacks_client_store::ws_suite::{ReceiverConnection, ReceiverSettings};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tokio::sync::{mpsc, watch};

struct Screen {
    form: ReceiverUi,
    view: watch::Receiver<ReceiverView>,
    commands: mpsc::Sender<SessionCommand>,
}
fn screen() -> (Harness<'static, Screen>, mpsc::Receiver<SessionCommand>) {
    let (commands, input) = mpsc::channel(8);
    let (_, view) = watch::channel(ReceiverView {
        settings: Some(ReceiverSettings::default()),
        ..Default::default()
    });
    let mut form = ReceiverUi::default();
    form.open(None, &view.borrow(), &commands);
    let screen = Screen {
        form,
        view,
        commands,
    };
    let harness = Harness::builder()
        .with_size(eframe::egui::vec2(900., 900.))
        .build_ui_state(
            |ui, s: &mut Screen| {
                s.form.poll(&s.view.borrow());
                s.form.show(ui.ctx(), &s.commands);
            },
            screen,
        );
    (harness, input)
}

#[test]
fn apply_submits_current_receiver_settings() {
    let (mut h, mut input) = screen();
    let request = input.try_recv().unwrap();
    if let SessionCommand::WsSuite(request) = request {
        request
            .reply
            .send(crate::ws_suite::ReceiverResponse::Interfaces(Ok(
                crate::ws_suite::interfaces::enumerate().unwrap(),
            )))
            .unwrap();
    } else {
        panic!("interface request");
    }
    h.step();
    h.get_by_label("Receive WS Suite updates").click();
    h.run();
    let now = h.ctx.input(|input| input.time);
    h.input_mut().time = Some(now + 0.31);
    h.step();
    h.get_by_label("Apply").click();
    h.run();
    let SessionCommand::WsSuite(request) = input.try_recv().unwrap() else {
        panic!("receiver request")
    };
    let ReceiverAction::Apply { next } = request.action else {
        panic!("Apply")
    };
    assert_eq!(
        next,
        ReceiverSettings {
            enabled: true,
            connection: ReceiverConnection::Unicast {
                address: "127.0.0.1".parse().unwrap(),
                port: 2237
            }
        }
    );
}

#[test]
fn mode_controls_and_cancel() {
    let (mut h, mut input) = screen();
    input.try_recv().unwrap();
    h.get_by_label("Receive WS Suite updates").click();
    h.run();
    h.get_by_label("Multicast").click();
    h.run();
    assert!(h.query_by_label("Group address").is_some());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(!h.state().form.is_open());
    assert!(input.try_recv().is_err());
}

#[test]
fn off_bypasses_unavailable_fields() {
    let (mut h, mut input) = screen();
    input.try_recv().unwrap();
    h.get_by_label("Receive WS Suite updates").click();
    h.run();
    h.get_by_label("Multicast").click();
    h.run();
    h.get_by_label("Receive WS Suite updates").click();
    h.run();
    let now = h.ctx.input(|input| input.time);
    h.input_mut().time = Some(now + 0.31);
    h.step();
    h.get_by_label("Apply").click();
    h.run();
    let SessionCommand::WsSuite(request) = input.try_recv().unwrap() else {
        panic!("receiver request")
    };
    let ReceiverAction::Apply { next } = request.action else {
        panic!("Apply")
    };
    assert_eq!(next, ReceiverSettings::default());
}

#[test]
fn unchanged_failed_apply_retries_and_closes_only_on_success() {
    let (mut h, mut input) = screen();
    let SessionCommand::WsSuite(request) = input.try_recv().unwrap() else {
        panic!("interface request")
    };
    request
        .reply
        .send(crate::ws_suite::ReceiverResponse::Interfaces(Ok(
            crate::ws_suite::interfaces::enumerate().unwrap(),
        )))
        .unwrap();
    h.step();
    h.get_by_label("Receive WS Suite updates").click();
    h.run();
    let now = h.ctx.input(|input| input.time);
    h.input_mut().time = Some(now + 0.31);
    h.step();
    h.get_by_label("Apply").click();
    h.run();
    let SessionCommand::WsSuite(request) = input.try_recv().unwrap() else {
        panic!("apply")
    };
    let failed = ReceiverView {
        settings: Some(ReceiverSettings {
            enabled: true,
            ..ReceiverSettings::default()
        }),
        reception: crate::ws_suite::ReceptionState::Failed("Port in use".into()),
        ..Default::default()
    };
    h.state_mut().view = watch::channel(failed.clone()).1;
    request
        .reply
        .send(crate::ws_suite::ReceiverResponse::Applied(failed.clone()))
        .unwrap();
    h.step();
    h.run();
    assert!(h.state().form.is_open());
    assert!(
        h.query_by_label("Reception unavailable. Port in use")
            .is_some()
    );
    assert!(!h.get_by_label("Apply").accesskit_node().is_disabled());
    h.get_by_label("Apply").click();
    h.run();
    let SessionCommand::WsSuite(request) = input.try_recv().unwrap() else {
        panic!("retry")
    };
    let ReceiverAction::Apply { next } = request.action else {
        panic!("Apply")
    };
    assert_eq!(next, failed.settings.clone().unwrap());
    request
        .reply
        .send(crate::ws_suite::ReceiverResponse::Applied(ReceiverView {
            settings: failed.settings,
            reception: crate::ws_suite::ReceptionState::Listening,
            ..Default::default()
        }))
        .unwrap();
    h.step();
    assert!(!h.state().form.is_open());
}

#[test]
fn local_scope_validation_identifies_the_interface_by_apply() {
    let (mut h, mut input) = screen();
    let SessionCommand::WsSuite(request) = input.try_recv().unwrap() else {
        panic!("interfaces")
    };
    request
        .reply
        .send(crate::ws_suite::ReceiverResponse::Interfaces(Ok(
            crate::ws_suite::interfaces::enumerate().unwrap(),
        )))
        .unwrap();
    h.step();
    h.run();
    h.get_by_label("Receive WS Suite updates").click();
    h.run();
    h.get_by_label("Multicast").click();
    h.run();
    h.get_by_label("Group address")
        .set_text_value_accesskit("ff02::114");
    h.run();
    let now = h.ctx.input(|input| input.time);
    h.input_mut().time = Some(now + 0.31);
    h.step();
    h.run();
    assert!(h.get_by_label("Apply").accesskit_node().is_disabled());
    assert!(
        h.query_by_label("Cannot apply: Network interface.")
            .is_some()
    );
}
