//! Connection form tests exercise actual submitted commands rather than protocol mocks.

use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

struct Host {
    view: EnrollmentUi,
    commands: mpsc::Sender<SessionCommand>,
    requests: mpsc::Receiver<SessionCommand>,
    updates: watch::Sender<EnrollmentStatus>,
    status: watch::Receiver<EnrollmentStatus>,
}

#[test]
fn connection_form_validates_submits_once_and_cancels_pending_work() {
    let (commands, requests) = mpsc::channel(8);
    let (updates, status) = watch::channel(EnrollmentStatus::default());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 800.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                host.view.show(ui.ctx(), &host.commands, &mut host.status);
            },
            Host {
                view: EnrollmentUi {
                    open: true,
                    server: String::new(),
                    ..Default::default()
                },
                commands,
                requests,
                updates,
                status,
            },
        );
    assert!(
        harness
            .get_by_label("Connect")
            .accesskit_node()
            .is_disabled()
    );
    harness.get_by_label("Server").focus();
    harness
        .get_by_label("Server")
        .type_text("https://clacks.example");
    harness.run();
    assert!(
        !harness
            .get_by_label("Connect")
            .accesskit_node()
            .is_disabled()
    );
    harness.get_by_label("Connect").click();
    harness.run();
    let SessionCommand::Enrollment(EnrollmentCommand::Connect { server, label }) =
        harness.state_mut().requests.try_recv().unwrap()
    else {
        panic!("connection request");
    };
    assert_eq!(server, "https://clacks.example");
    assert_eq!(label, "Desktop");
    assert!(
        harness
            .get_by_label("Connect")
            .accesskit_node()
            .is_disabled()
    );
    harness.get_by_label("Connect").click();
    harness.step();
    assert!(harness.state_mut().requests.try_recv().is_err());
    harness
        .state()
        .updates
        .send_replace(EnrollmentStatus::Saving);
    harness.run();
    assert!(
        harness
            .get_by_label("Server")
            .accesskit_node()
            .is_read_only()
    );
    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(matches!(
        harness.state_mut().requests.try_recv().unwrap(),
        SessionCommand::Enrollment(EnrollmentCommand::Cancel)
    ));
    assert!(!harness.state().view.open);
    harness.state_mut().view.open = true;
    harness
        .state()
        .updates
        .send_replace(EnrollmentStatus::Failed {
            enrolled: false,
            message: "Refresh authorization was rejected".into(),
            details: Some("HTTP 500".into()),
        });
    harness.run();
    assert!(
        !harness
            .get_by_label("Connect")
            .accesskit_node()
            .is_disabled(),
        "a failed connection must be retryable through Connect"
    );
    harness.get_by_label("Connect").click();
    harness.run();
    assert!(matches!(harness.state_mut().requests.try_recv().unwrap(),
        SessionCommand::Enrollment(EnrollmentCommand::Connect { server, label }) if label == "Desktop" && server == "https://clacks.example"));
    harness
        .state()
        .updates
        .send_replace(EnrollmentStatus::Ready);
    harness.run();
    assert!(!harness.state().view.is_open());
    harness.state_mut().view.open(None);
    harness.run();
    assert!(
        harness
            .get_by_label("Server")
            .accesskit_node()
            .is_read_only()
    );
    assert!(
        !harness
            .get_by_label("Reauthorize")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn fresh_connection_can_submit_hosted_defaults() {
    let (commands, mut requests) = mpsc::channel(8);
    let (updates, mut status) = watch::channel(EnrollmentStatus::default());
    let mut view = EnrollmentUi::default();
    view.open(None);
    // Startup publishes the lack of a binding; this must not erase the default.
    updates.send_replace(EnrollmentStatus::Idle {
        server: None,
        enrolled: false,
    });
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 800.0))
        .build_ui(|ui| view.show(ui.ctx(), &commands, &mut status));
    harness.run();
    assert!(
        !harness
            .get_by_label("Connect")
            .accesskit_node()
            .is_disabled()
    );
    harness.get_by_label("Connect").click();
    harness.run();
    assert!(matches!(requests.try_recv().unwrap(),
        SessionCommand::Enrollment(EnrollmentCommand::Connect { server, label })
        if server == "https://clacks.w7hex.com" && label == "Desktop"));
}

#[test]
fn installed_server_replaces_untouched_hosted_default_when_dialog_opens_first() {
    let (commands, mut requests) = mpsc::channel(8);
    let (updates, mut status) = watch::channel(EnrollmentStatus::default());
    let mut view = EnrollmentUi::default();
    view.open(None);
    updates.send_replace(EnrollmentStatus::Idle {
        server: Some("https://self-hosted.example".into()),
        enrolled: true,
    });
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 800.0))
        .build_ui(|ui| view.show(ui.ctx(), &commands, &mut status));
    harness.run();
    assert!(
        harness
            .get_by_label("Server")
            .accesskit_node()
            .is_read_only()
    );
    harness.get_by_label("Reauthorize").click();
    harness.run();
    assert!(matches!(requests.try_recv().unwrap(),
        SessionCommand::Enrollment(EnrollmentCommand::Connect { server, .. })
        if server == "https://self-hosted.example"));
}
