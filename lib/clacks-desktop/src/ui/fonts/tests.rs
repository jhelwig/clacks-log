//! Choice/response correlation and rendering at Clacks's native font boundary.

use super::*;

fn bundled_face() -> FontData {
    FontDefinitions::default()
        .font_data
        .get("Ubuntu-Light")
        .unwrap()
        .as_ref()
        .clone()
}

#[test]
fn font_result_for_previous_choice_cannot_replace_current_preview() {
    let old = FontChoice::Installed {
        family: "Earlier family".into(),
    };
    let current = FontChoice::Installed {
        family: "Current family".into(),
    };
    let (reply, response) = oneshot::channel();
    let mut state = RoleState {
        requested: Some(old.clone()),
        request: Some(FontRequest {
            choice: old,
            reply: response,
        }),
        ..Default::default()
    };
    reply.send(Ok(bundled_face())).unwrap();
    let (commands, mut requests) = mpsc::channel(8);

    update_role(&mut state, &current, &commands);

    assert!(state.data.is_none());
    assert_eq!(state.requested, Some(current));
    assert!(
        matches!(requests.try_recv(), Ok(SessionCommand::Font(FontOperation::Load { family, .. })) if family == "Current family")
    );
}

#[test]
fn missing_selected_family_uses_default_without_erasing_choice() {
    let choice = FontChoice::Installed {
        family: "Missing family".into(),
    };
    let (reply, response) = oneshot::channel();
    let mut state = RoleState {
        requested: Some(choice.clone()),
        request: Some(FontRequest {
            choice: choice.clone(),
            reply: response,
        }),
        ..Default::default()
    };
    let (commands, mut requests) = mpsc::channel(8);
    reply
        .send(Err(anyhow::anyhow!("Unavailable family")))
        .unwrap();

    update_role(&mut state, &choice, &commands);
    update_role(&mut state, &choice, &commands);

    assert_eq!(state.requested, Some(choice));
    assert!(state.data.is_none());
    assert!(state.notice.as_ref().unwrap().contains("bundled default"));
    assert!(
        requests.try_recv().is_err(),
        "failure must not create a per-frame retry loop"
    );
}

#[test]
fn font_admission_failure_retains_choice_and_reports_recovery() {
    let choice = FontChoice::Installed {
        family: "Selected family".into(),
    };
    let (commands, requests) = mpsc::channel(1);
    drop(requests);
    let mut state = RoleState::default();

    update_role(&mut state, &choice, &commands);
    update_role(&mut state, &choice, &commands);

    assert_eq!(state.requested, Some(choice));
    assert!(state.notice.is_some());
    assert!(state.request.is_none());
}

#[test]
fn font_face_is_used_by_radio_field_without_rewriting_text() {
    let context = egui::Context::default();
    let mut fonts = FontsUi::default();
    let choice = FontChoice::Installed {
        family: "Selected family".into(),
    };
    let preferences = AppearancePreferences {
        radio_font: choice.clone(),
        ..Default::default()
    };
    let (reply, response) = oneshot::channel();
    fonts.radio.requested = Some(choice.clone());
    fonts.radio.request = Some(FontRequest {
        choice,
        reply: response,
    });
    reply.send(Ok(bundled_face())).unwrap();
    let (commands, _requests) = mpsc::channel(8);
    let mut callsign = "K7ABC/P".to_owned();

    fonts.update(&context, &preferences, &commands, false);
    super::super::theme::install(&context, &preferences);
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        ui.add(egui::TextEdit::singleline(&mut callsign).font(egui::TextStyle::Monospace));
    });

    assert_eq!(callsign, "K7ABC/P");
    context.fonts_mut(|view| {
        let font = egui::FontId::new(13.0, FontFamily::Name("clacks.radio".into()));
        assert_ne!(
            view.glyph_width(&font, 'I'),
            view.glyph_width(&font, 'W'),
            "the selected proportional fixture must displace the bundled monospace face"
        );
    });
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text() == "K7ABC/P" && text.galley.job.sections.iter().any(|section| section.format.font_id.family == FontFamily::Name("clacks.radio".into())))));
    output.drop_without_applying_deltas();
}

#[test]
fn selected_font_failure_is_visible_without_recording_the_requested_family() {
    struct Captured(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_ansi(false)
        .without_time()
        .with_writer(move || Captured(writer.clone()))
        .finish();
    let (reply, result) = tokio::sync::oneshot::channel();

    tracing::subscriber::with_default(subscriber, || {
        FontOperation::Load {
            family: "Clacks-private-missing-family-7ed9\0".into(),
            reply,
        }
        .run();
    });

    assert!(result.blocking_recv().unwrap().is_err());
    let diagnostic = String::from_utf8(captured.lock().unwrap().clone()).unwrap();
    assert!(diagnostic.contains("desktop.selected_font_unavailable"));
    assert!(!diagnostic.contains("Clacks-private-missing-family"));
}
