//! Real table interactions preserve contact and column identity.

use super::*;
use crate::ui::tests::fixtures;
use clacks_client_store::read::{contacts::ContactColumn, history::HistoryRow};
use egui_kittest::{Harness, kittest::Queryable};

struct Host {
    page: ContactPage,
    preferences: preferences::ContactTablePreferences,
    selected: BTreeSet<QsoId>,
    focused: Option<QsoId>,
    pending_focus: Option<QsoId>,
    opened: Option<QsoId>,
}

fn mount() -> Harness<'static, Host> {
    let rows = (0..40)
        .map(|n| {
            let mut qso = fixtures::contact();
            qso.contacted_station.callsign =
                clacks_model::scalar::RecordedCallsign::new(&format!("K1T{n:03}")).unwrap();
            HistoryRow {
                qso,
                pending: true,
                review: None,
            }
        })
        .collect();
    Harness::builder()
        .with_size(egui::vec2(1100.0, 240.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                let action = table::show(
                    ui,
                    Some(&host.page),
                    &mut host.preferences,
                    &mut host.selected,
                    &mut host.focused,
                    &mut host.pending_focus,
                );
                if action.open.is_some() {
                    host.opened = action.open;
                }
            },
            Host {
                page: ContactPage {
                    rows,
                    has_more: false,
                },
                preferences: Default::default(),
                selected: Default::default(),
                focused: None,
                pending_focus: None,
                opened: None,
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
    let expected = ui.state().page.rows.get(12).unwrap().qso.id;
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
