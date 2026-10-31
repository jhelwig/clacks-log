use super::*;
use crate::ui::tests::fixtures;
use clacks_model::tag::LogbookTag;
use clacks_sync::record::RecordValue;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn picker_preserves_mixed_intent_through_search() {
    let main = LogbookTag::new(Default::default(), "Main".into());
    let activity = LogbookTag::new(clacks_model::ids::TagId::new(), "Activity".into());
    let portable = LogbookTag::new(clacks_model::ids::TagId::new(), "Portable".into());
    let mut catalog = clacks_client_store::logbooks::LogbookSnapshot {
        selected_view: Default::default(),
        tags: [&main, &activity, &portable]
            .iter()
            .map(|tag| fixtures::row(RecordValue::Tag(Box::new((*tag).clone()))))
            .collect(),
    };
    for n in 0..244 {
        let tag = LogbookTag::new(clacks_model::ids::TagId::new(), format!("Logbook {n:03}"));
        catalog
            .tags
            .push(fixtures::row(RecordValue::Tag(Box::new(tag))));
    }
    let picker =
        MembershipPicker::new(&catalog, &[[main.id, activity.id].into(), [main.id].into()]);
    let mut ui = Harness::builder().build_ui_state(
        |ui, picker| {
            show_contents(ui, picker);
        },
        picker,
    );
    assert!(ui.state().intent().add.is_empty());
    ui.get_by_label("Activity").click();
    ui.run();
    assert_eq!(ui.state().intent().add, [activity.id].into());
    ui.get_by_label("Activity").click();
    ui.run();
    assert_eq!(ui.state().intent().remove, [activity.id].into());
    ui.get_by_label("Activity").click();
    ui.run();
    assert!(!ui.state().is_changed());
    ui.get_by_label("Search logbooks").click();

    ui.get_by_label("Search logbooks").type_text("Portable");
    ui.run();
    assert_eq!(ui.state().search, "Portable");
    ui.get_by_label("Portable").click();
    ui.run();
    assert_eq!(ui.state().intent().add, [portable.id].into());
    ui.get_by_label("Search logbooks")
        .set_text_value_accesskit("Activity");
    ui.run();
    assert_eq!(ui.state().intent().add, [portable.id].into());
    ui.get_by_label("Activity").click();
    ui.run();
    assert_eq!(ui.state().intent().add, [portable.id, activity.id].into());
    assert!(ui.state().intent().remove.is_empty());
}

#[test]
fn inline_picker_respects_its_resolved_value_column() {
    struct Host {
        picker: MembershipPicker,
        available_right: f32,
        content_right: f32,
    }
    let tag = LogbookTag::new(Default::default(), "Portable".into());
    let mut catalog = fixtures::snapshot().logbooks;
    catalog
        .tags
        .push(fixtures::row(RecordValue::Tag(Box::new(tag))));
    let ui = Harness::builder()
        .with_size(egui::vec2(1200.0, 500.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                ui.allocate_ui_with_layout(
                    egui::vec2(220.0, 350.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        host.available_right = ui.max_rect().right();
                        show_contents(ui, &mut host.picker);
                        host.content_right = ui.min_rect().right();
                    },
                );
            },
            Host {
                picker: MembershipPicker::new(&catalog, &[Default::default()]),
                available_right: 0.0,
                content_right: 0.0,
            },
        );
    assert!(
        ui.state().content_right <= ui.state().available_right + 1.0,
        "membership choices escaped their column"
    );
}
