//! Real table interactions preserve contact and column identity.

use super::*;
use crate::ui::{tests::fixtures, theme};
use clacks_client_store::read::contacts::ContactPage;
use clacks_client_store::read::{
    contacts::{ContactColumn, SortDirection},
    history::HistoryRow,
};
use egui_kittest::{Harness, kittest::Queryable};

struct Host {
    data: data::TableData,
    needed: BTreeSet<u32>,
    selected_only: bool,
    preferences: preferences::ContactTablePreferences,
    selected: BTreeSet<QsoId>,
    focused: Option<QsoId>,
    pending_focus: Option<PendingFocus>,
    opened: Option<QsoId>,
    entry: String,
}

fn mount() -> Harness<'static, Host> {
    let rows = (0..40)
        .map(|n| {
            let mut qso = fixtures::contact();
            crate::test_support::edit(&mut qso, |qso| {
                qso.contacted_station.callsign =
                    clacks_model::scalar::RecordedCallsign::new(&format!("K1T{n:03}")).unwrap();
                Ok(())
            });
            clacks_client_store::read::contacts::ContactRow {
                facts: Default::default(),
                working: HistoryRow {
                    qso,
                    pending: true,
                    review: None,
                },
            }
        })
        .collect();
    Harness::builder()
        .with_size(egui::vec2(1100.0, 240.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                let label = ui.label("Entry callsign");
                ui.text_edit_singleline(&mut host.entry)
                    .labelled_by(label.id);
                let action = show(
                    ui,
                    TableContent {
                        data: &host.data,
                        selected_only: host.selected_only,
                        reset_scroll: false,
                        loading_message: None,
                    },
                    &mut host.preferences,
                    &mut host.selected,
                    &mut host.focused,
                    &mut host.pending_focus,
                    None,
                );
                host.needed = action.needed;
                if action.open.is_some() {
                    host.opened = action.open;
                }
            },
            Host {
                data: {
                    let mut data = data::TableData::default();
                    data.accept(
                        0,
                        ContactPage {
                            rows,
                            has_more: false,
                            matching_total: 40,
                            selected_total: 0,
                            selected_outside_filter: 0,
                        },
                        true,
                    );
                    data
                },
                needed: Default::default(),
                selected_only: false,
                preferences: Default::default(),
                selected: Default::default(),
                focused: None,
                pending_focus: None,
                opened: None,
                entry: String::new(),
            },
        )
}

#[test]
fn keyboard_navigation_scrolls_to_and_opens_the_contact_by_identity() {
    let mut ui = mount();
    ui.get_by_label("K1T000").click();
    ui.run();
    for _ in 0..12 {
        ui.key_press(egui::Key::ArrowDown);
        ui.run();
    }
    let expected = ui.state().data.row(12).unwrap().working.qso.id();
    assert_eq!(ui.state().focused, Some(expected));
    assert_eq!(ui.state().selected, [expected].into());
    assert!(ui.query_by_label("K1T012").is_some());
    ui.key_press(egui::Key::Enter);
    ui.run();
    assert_eq!(ui.state().opened, Some(expected));
}

#[test]
fn reordering_columns_preserves_widths_by_column_identity() {
    let mut ui = mount();
    ui.state_mut()
        .preferences
        .columns
        .retain(|v| matches!(v.column, ContactColumn::Callsign | ContactColumn::Start));
    for column in &mut ui.state_mut().preferences.columns {
        column.width = if column.column == ContactColumn::Start {
            250.0
        } else {
            100.0
        };
    }
    ui.run();
    let before = ui.state().preferences.columns.clone();
    ui.state_mut().preferences.columns.swap(0, 1);
    ui.run();
    for column in before {
        let saved = ui
            .state()
            .preferences
            .columns
            .iter()
            .find(|v| v.column == column.column)
            .unwrap();
        assert!((saved.width - column.width).abs() < 1.0);
    }
}

#[test]
fn tab_focus_can_open_a_contact_without_a_pointer_click() {
    let mut ui = mount();
    for _ in 0..24 {
        ui.key_press(egui::Key::Tab);
        ui.run();
        if ui.state().focused.is_some() {
            break;
        }
    }
    let focused = ui.state().focused.expect("Tab reaches a contact row");
    ui.key_press(egui::Key::Enter);
    ui.run();
    assert_eq!(ui.state().opened, Some(focused));
}

#[test]
fn full_cell_headers_sort_and_configure_columns_even_without_contacts() {
    let mut ui = mount();
    ui.set_size(egui::vec2(1100.0, 650.0));
    ui.state_mut()
        .data
        .accept(0, fixtures::contact_page(0, 0, 0), true);
    ui.run();
    assert!(ui.query_by_label("Columns").is_none());
    let start = ui
        .get_by_role_and_label(egui::Role::ColumnHeader, "Start · UTC")
        .rect();
    let call = ui
        .get_by_role_and_label(egui::Role::ColumnHeader, "Callsign")
        .rect();
    assert!(
        (call.right() - start.left()).abs() < 1.0,
        "header cells must meet at the column boundary: {call:?}, {start:?}"
    );
    for header in [call, start] {
        assert!(
            ui.output().shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Rect(rect)
                    if rect.fill == theme::palette(ui.ctx.theme()).header
                        && rect.rect.contains_rect(header)
            )),
            "the header surface must fill its entire interactive cell"
        );
    }
    assert!(
        start.width() >= 175.0,
        "the entire column header is interactive"
    );
    ui.get_by_role_and_label(egui::Role::ColumnHeader, "Start · UTC")
        .click();
    ui.run();
    assert_eq!(
        ui.state().preferences.order.direction,
        SortDirection::Ascending
    );
    assert!(ui.state().selected.is_empty());

    ui.get_by_label("Callsign").click_secondary();
    ui.run();
    ui.get_by_label("Own call").click();
    ui.run();
    assert!(
        ui.state()
            .preferences
            .columns
            .iter()
            .find(|c| c.column == ContactColumn::OwnCall)
            .unwrap()
            .visible
    );
    ui.key_press(egui::Key::Escape);
    ui.run();
    assert!(ui.query_by_label("Own call").is_some());

    ui.get_by_label("Callsign").click_secondary();
    ui.run();
    ui.get_by_label("Move column right").click();
    ui.run();
    assert_eq!(
        ui.state().preferences.columns.first().unwrap().column,
        ContactColumn::Start
    );
    ui.get_by_label("Callsign").click_secondary();
    ui.run();
    ui.get_by_label("Reset table defaults").click();
    ui.run();
    assert_eq!(
        ui.state().preferences.columns.first().unwrap().column,
        ContactColumn::Callsign
    );
    assert!(
        !ui.state()
            .preferences
            .columns
            .iter()
            .find(|c| c.column == ContactColumn::OwnCall)
            .unwrap()
            .visible
    );

    // A previously saved all-hidden layout must still have a recovery target.
    for column in &mut ui.state_mut().preferences.columns {
        column.visible = false;
    }
    ui.run();
    ui.get_by_label("Right-click to choose columns")
        .click_secondary();
    ui.run();
    ui.get_by_label("Own call").click();
    ui.key_press(egui::Key::Escape);
    ui.run();
    assert!(ui.query_by_label("Own call").is_some());
}

#[test]
fn distant_rows_request_only_their_internal_page_and_open_by_identity() {
    let mut ui = mount();
    ui.state_mut()
        .data
        .accept(0, fixtures::contact_page(0, 100, 1000), true);
    ui.state_mut().pending_focus = Some(PendingFocus {
        index: 734,
        owner: None,
    });
    ui.run_steps(3);
    assert!(ui.state().needed.contains(&700));
    assert!(ui.state().needed.iter().all(|offset| *offset >= 600));
    assert!(ui.state().pending_focus.is_some());
    assert!(ui.state().opened.is_none());

    let page = fixtures::contact_page(700, 100, 1000);
    let expected = page.rows.get(34).unwrap().working.qso.id();
    ui.state_mut().data.accept(700, page, false);
    ui.run();
    assert!(ui.query_by_label("K1T734").is_some());
    assert_eq!(ui.state().focused, Some(expected));
    ui.key_press(egui::Key::Enter);
    ui.run();
    assert_eq!(ui.state().opened, Some(expected));
}

#[test]
fn keyboard_crosses_an_unloaded_boundary_without_opening_the_previous_contact() {
    let mut ui = mount();
    ui.state_mut()
        .data
        .accept(0, fixtures::contact_page(0, 100, 250), true);
    ui.state_mut().pending_focus = Some(PendingFocus {
        index: 99,
        owner: None,
    });
    ui.run();
    let previous = ui.state().focused;
    ui.key_press(egui::Key::ArrowDown);
    ui.run_steps(3);
    assert_eq!(
        ui.state().pending_focus.map(|target| target.index),
        Some(100)
    );
    assert!(ui.state().needed.contains(&100));
    ui.key_press(egui::Key::Enter);
    ui.run_steps(2);
    assert!(ui.state().opened.is_none());
    assert_eq!(ui.state().focused, previous);

    let page = fixtures::contact_page(100, 100, 250);
    let expected = page.rows.first().unwrap().working.qso.id();
    ui.state_mut().data.accept(100, page, false);
    ui.run();
    assert_eq!(ui.state().selected, [expected].into());
    ui.key_press(egui::Key::Enter);
    ui.run();
    assert_eq!(ui.state().opened, Some(expected));
}

#[test]
fn show_selected_scrolls_the_selected_set_independently_of_expression_count() {
    let mut ui = mount();
    let mut page = fixtures::contact_page(0, 100, 3);
    page.selected_total = 210;
    page.selected_outside_filter = 207;
    ui.state_mut().data.accept(0, page, true);
    ui.state_mut().selected_only = true;
    ui.state_mut().pending_focus = Some(PendingFocus {
        index: 209,
        owner: None,
    });
    ui.run_steps(3);
    assert!(ui.state().needed.contains(&200));
    let mut page = fixtures::contact_page(200, 10, 3);
    page.selected_total = 210;
    page.selected_outside_filter = 207;
    ui.state_mut().data.accept(200, page, false);
    ui.run();
    assert!(ui.query_by_label("K1T209").is_some());
    ui.key_press(egui::Key::ArrowDown);
    ui.run();
    assert!(ui.state().pending_focus.is_none());
    assert!(ui.query_by_label("K1T209").is_some());
}

#[test]
fn delayed_keyboard_target_does_not_take_focus_from_unfinished_entry() {
    let mut ui = mount();
    ui.state_mut()
        .data
        .accept(0, fixtures::contact_page(0, 100, 250), true);
    ui.state_mut().pending_focus = Some(PendingFocus {
        index: 99,
        owner: None,
    });
    ui.run();
    ui.key_press(egui::Key::ArrowDown);
    ui.run_steps(3);
    assert_eq!(
        ui.state().pending_focus.map(|target| target.index),
        Some(100)
    );
    ui.get_by_role_and_label(egui::Role::TextInput, "Entry callsign")
        .click();
    ui.run_steps(3);
    ui.state_mut()
        .data
        .accept(100, fixtures::contact_page(100, 100, 250), false);
    ui.run();
    assert!(
        ui.get_by_role_and_label(egui::Role::TextInput, "Entry callsign")
            .is_focused()
    );
    ui.key_press(egui::Key::ArrowLeft);
    ui.run();
    assert!(
        ui.get_by_role_and_label(egui::Role::TextInput, "Entry callsign")
            .is_focused()
    );
}
