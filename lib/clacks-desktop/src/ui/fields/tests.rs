//! Edited, validation and keyboard focus remain simultaneous in both Clacks palettes.

use super::*;

#[test]
fn edited_invalid_and_focused_states_coexist() {
    for appearance in [
        super::super::preferences::Appearance::Light,
        super::super::preferences::Appearance::Dark,
    ] {
        let context = egui::Context::default();
        theme::install(
            &context,
            &super::super::preferences::AppearancePreferences {
                appearance,
                ..Default::default()
            },
        );
        let colors = theme::palette(context.theme());
        let id = egui::Id::unique("test.power");
        context.memory_mut(|memory| memory.request_focus(id));
        let mut value = "invalid".to_owned();
        let errors = vec![FieldError {
            field: "Transmit power".into(),
            message: "Enter positive watts.".into(),
        }];
        let mut bounds = egui::Rect::NOTHING;
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
            bounds = response.rect;
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
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::epaint::Shape::Rect(rect) if rect.rect == bounds && rect.fill == colors.edited_fill
        )), "edited background must survive keyboard focus");
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::epaint::Shape::LineSegment { points, stroke } if stroke.color == colors.edited_edge
                && stroke.width == 2.0 && points[0].x == bounds.left() + 2.0
                && points[1].x == points[0].x
        )), "edited edge must remain independent of validation");
        assert!(
            output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::epaint::Shape::Rect(rect) if rect.rect == bounds.expand(1.0)
                    && rect.stroke.color == colors.error && rect.stroke.width == 1.0
            )),
            "validation outline must coexist with the edited background"
        );
        output.drop_without_applying_deltas();
    }
}
