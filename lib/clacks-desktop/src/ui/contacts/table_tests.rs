//! Contact identity, keyboard reachability and column-width ownership in the real table.

use super::*;
use crate::logging::{form::ContactForm, time::ContactTimestamp};
use clacks_client_store::read::{
    contacts::{ContactColumn, ContactOrder},
    history::HistoryRow,
};
use clacks_model::scalar::UtcSecond;
use eframe::egui::{Event, Key, Modifiers, PointerButton};

struct Window {
    context: egui::Context,
    page: ContactPage,
    preferences: preferences::ContactTablePreferences,
    selected: BTreeSet<QsoId>,
    focused: Option<QsoId>,
    pending_focus: Option<QsoId>,
}

struct Frame {
    output: egui::FullOutput,
    action: table::TableAction,
}

impl Window {
    fn new() -> anyhow::Result<Self> {
        let mut rows = Vec::new();
        for n in 0..80 {
            let mut form = ContactForm::empty();
            form.own.callsign = "W7HEX".into();
            form.dx.callsign = format!("K1T{n:03}");
            form.mode = "SSB".into();
            form.primary.band = "20m".into();
            form.start = ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(n)));
            let qso = form
                .build(QsoId::new(), &DefinitionSet::default())
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            rows.push(HistoryRow {
                qso,
                pending: true,
                review: None,
            });
        }
        Ok(Self {
            context: Default::default(),
            page: ContactPage {
                rows,
                has_more: false,
            },
            preferences: Default::default(),
            selected: Default::default(),
            focused: None,
            pending_focus: None,
        })
    }

    fn frame(&mut self, events: Vec<Event>) -> Frame {
        let mut action = table::TableAction::default();
        let mut output = self.context.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 240.0),
                )),
                ..Default::default()
            },
            |ui| {
                action = table::show(
                    ui,
                    Some(&self.page),
                    &mut self.preferences,
                    &mut self.selected,
                    &mut self.focused,
                    &mut self.pending_focus,
                );
            },
        );
        output.textures_delta.clear();
        Frame { output, action }
    }

    fn key(&mut self, key: Key) -> Frame {
        let frame = self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        frame
    }
}

#[test]
fn keyboard_navigation_scrolls_to_and_opens_the_selected_contact() -> anyhow::Result<()> {
    let mut window = Window::new()?;
    window.frame(vec![]);
    let frame = window.frame(vec![]);
    let position = frame
        .output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == "K1T000" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .expect("first contact is visible");
    window.frame(vec![
        Event::PointerMoved(position),
        Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
    ]);
    window.frame(vec![Event::PointerButton {
        pos: position,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    for _ in 0..30 {
        window.key(Key::ArrowDown);
    }
    let expected = window.page.rows.get(30).unwrap().qso.id;
    assert_eq!(
        window.focused,
        Some(expected),
        "actual row {:?}",
        window
            .focused
            .and_then(|id| window.page.rows.iter().position(|row| row.qso.id == id))
    );
    assert_eq!(window.key(Key::Enter).action.open, Some(expected));
    Ok(())
}

#[test]
fn reordering_columns_preserves_widths_by_column_identity() -> anyhow::Result<()> {
    let mut window = Window::new()?;
    window
        .preferences
        .columns
        .retain(|v| matches!(v.column, ContactColumn::Callsign | ContactColumn::Start));
    for column in &mut window.preferences.columns {
        column.width = if column.column == ContactColumn::Start {
            250.0
        } else {
            100.0
        };
    }
    window.preferences.order = ContactOrder::default();
    window.frame(vec![]);
    window.frame(vec![]);
    let before: Vec<_> = window
        .preferences
        .columns
        .iter()
        .map(|v| (v.column, v.width))
        .collect();
    window.preferences.columns.swap(0, 1);
    window.frame(vec![]);
    window.frame(vec![]);
    for (column, width) in before {
        assert!(
            (window
                .preferences
                .columns
                .iter()
                .find(|v| v.column == column)
                .unwrap()
                .width
                - width)
                .abs()
                < 1.0,
            "width belongs to {column:?}"
        );
    }
    Ok(())
}

#[test]
fn tab_focus_can_open_a_contact_without_a_pointer_click() -> anyhow::Result<()> {
    let mut window = Window::new()?;
    window.frame(vec![]);
    for _ in 0..24 {
        window.key(Key::Tab);
        if window.focused.is_some() {
            break;
        }
    }
    let focused = window.focused.expect("Tab reaches a contact row");
    assert_eq!(window.key(Key::Enter).action.open, Some(focused));
    Ok(())
}
