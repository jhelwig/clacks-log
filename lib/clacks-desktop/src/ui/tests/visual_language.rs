//! Workbench colors, control styling and typography in rendered application frames.

use super::logbooks_support::LiveWindow;
use super::*;
use crate::ui::preferences::{Appearance, AppearancePreferences, STORAGE_KEY};
use eframe::egui::{Color32, epaint::RectShape};

fn text_bounds(output: &egui::FullOutput, label: &str) -> egui::Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                Some(text.visual_bounding_rect())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {label}"))
}

fn button_frame<'a>(output: &'a egui::FullOutput, label: &str) -> &'a RectShape {
    let text = text_bounds(output, label);
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Rect(rect)
                if rect.rect.contains(text.center()) && rect.fill != Color32::TRANSPARENT =>
            {
                Some(rect)
            }
            _ => None,
        })
        .min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
        .unwrap()
}

fn font_size(output: &egui::FullOutput, label: &str) -> f32 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == label => text
                .galley
                .job
                .sections
                .first()
                .map(|section| section.format.font_id.size),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {label}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn actual_app_uses_reference_surfaces_control_treatment_and_type_hierarchy()
-> anyhow::Result<()> {
    let mut app = LiveWindow::new().await?;
    for appearance in [Appearance::Dark, Appearance::Light] {
        let mut storage = Storage::default();
        eframe::set_value(
            &mut storage,
            STORAGE_KEY,
            &AppearancePreferences {
                appearance,
                ..Default::default()
            },
        );
        app.window
            .app
            .restore_appearance(&app.window.context, Some(&storage));
        app.frame(vec![]);
        app.frame(vec![]);
        let background = app.window.context.content_rect().center();
        for pressed in [true, false] {
            app.frame(vec![
                egui::Event::PointerMoved(background),
                egui::Event::PointerButton {
                    pos: background,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        let workspace = match appearance {
            Appearance::Dark => Color32::from_rgb(29, 34, 40),
            _ => Color32::from_rgb(240, 241, 245),
        };
        assert!(app.output.shapes.iter().any(|shape|matches!(&shape.shape,egui::epaint::Shape::Rect(rect) if rect.fill==workspace && rect.rect.contains(app.window.context.content_rect().center()))),"workspace surface differs from the reference at {appearance:?}");
        let manage = button_frame(&app.output, "Manage logbooks…");
        let input = match appearance {
            Appearance::Dark => Color32::from_rgb(29, 34, 38),
            _ => Color32::from_rgb(245, 246, 248),
        };
        assert_eq!(manage.fill, input);
        assert_eq!(manage.corner_radius, egui::CornerRadius::same(2));
        assert_eq!(manage.stroke.width, 1.0);
        assert_eq!(font_size(&app.output, "Clacks Log"), 14.0);
        assert_eq!(font_size(&app.output, "Logbook"), 11.0);
        assert_eq!(font_size(&app.output, "Manage logbooks…"), 12.0);

        app.click("Manage logbooks…");
        app.click("New logbook");
        app.text("Portable");
        assert!(
            (text_bounds(&app.output, "Save logbook").center().y
                - text_bounds(&app.output, "Cancel edits").center().y)
                .abs()
                < text_bounds(&app.output, "Save logbook").height(),
            "Save and Cancel must share the action row"
        );
        let save = button_frame(&app.output, "Save logbook");
        let accent = match appearance {
            Appearance::Dark => Color32::from_rgb(173, 214, 231),
            _ => Color32::from_rgb(50, 103, 129),
        };
        assert_eq!(
            save.fill, accent,
            "primary action lacks the reference emphasis"
        );
        assert_eq!(save.corner_radius, egui::CornerRadius::same(2));
        app.click("Cancel edits");
        app.click("Close manager");
    }
    app.close().await
}
