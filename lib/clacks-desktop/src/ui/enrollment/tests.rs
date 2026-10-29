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
    let SessionCommand::Enrollment(EnrollmentCommand::Begin {
        server,
        label,
        destination,
    }) = harness.state_mut().requests.try_recv().unwrap()
    else {
        panic!("connection request");
    };
    assert_eq!(server, "https://clacks.example");
    assert_eq!(label, "Desktop");
    assert!(destination.is_none());
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
            message: "Refresh authorization was rejected".into(),
            resume_available: true,
            discard_available: false,
            destination: None,
            destination_change_available: false,
        });
    harness.run();
    harness.get_by_label("Authorize again in browser").click();
    harness.run();
    assert!(matches!(harness.state_mut().requests.try_recv().unwrap(),
        SessionCommand::Enrollment(EnrollmentCommand::AuthorizeAgain { label }) if label == "Desktop"));

    assert!(harness.state_mut().requests.try_recv().is_err());
    harness
        .state()
        .updates
        .send_replace(EnrollmentStatus::Failed {
            message: "Choose another destination".into(),
            resume_available: true,
            discard_available: false,
            destination: Some("/occupied-account".into()),
            destination_change_available: true,
        });
    harness.run();
    harness.get_by_label("Workspace directory").focus();
    harness.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
    harness
        .get_by_label("Workspace directory")
        .type_text("/new-account");
    harness.get_by_label("Use this destination").click();
    harness.run();
    assert!(matches!(harness.state_mut().requests.try_recv().unwrap(),
        SessionCommand::Enrollment(EnrollmentCommand::SetDestination { destination: Some(path) }) if path == std::path::Path::new("/new-account")));
}
