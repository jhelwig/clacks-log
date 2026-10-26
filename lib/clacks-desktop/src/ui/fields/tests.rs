//! Product field states stay independent while using the native text widget.

use super::*;

#[test]
fn edited_invalid_and_focused_states_coexist() {
    let context = egui::Context::default();
    let id = egui::Id::unique("test.power");
    context.memory_mut(|memory| memory.request_focus(id));
    let mut value = "invalid".to_owned();
    let errors = vec![FieldError {
        field: "Transmit power".into(),
        message: "Enter positive watts.".into(),
    }];
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        let response = text(
            ui,
            id,
            "Transmit power",
            &mut value,
            FieldPresentation {
                edited: true,
                read_only: false,
                errors: &errors,
                validation_label: None,
                radio_data: true,
            },
        );
        assert!(response.has_focus());
    });

    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        })
        .collect();
    assert!(labels.contains(&"Not applied"));
    assert!(labels.contains(&"Enter positive watts."));
    assert!(labels.contains(&"invalid"));
    output.drop_without_applying_deltas();
}
