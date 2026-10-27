//! Real bounded history rows with native IDs, selection and column presentation.

use super::{
    preferences::{self, ContactTablePreferences},
    *,
};
use clacks_client_store::read::contacts::{ContactColumn, SortDirection};
use clacks_model::recognition::mode::ModeIdentity;
use egui_extras::{Column, TableBuilder};

/// A contact explicitly activated from this table.
#[derive(Default)]
pub(super) struct TableAction {
    /// Stable identity to load, independent of row order.
    pub open: Option<QsoId>,
}

/// Display current rows; sorting requests change the SQL query rather than sorting payloads.
pub(super) fn show(
    ui: &mut egui::Ui,
    page: Option<&ContactPage>,
    preferences: &mut ContactTablePreferences,
    selected: &mut BTreeSet<QsoId>,
    focused: &mut Option<QsoId>,
    pending_focus: &mut Option<QsoId>,
) -> TableAction {
    let mut action = TableAction::default();
    let mut row_ids = std::collections::BTreeMap::new();
    let mut native_focus = None;

    let mut move_column = None;
    let mut reset = false;
    ui.menu_button("Columns", |ui| {
        let count = preferences.columns.len();
        for (index, column) in preferences.columns.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut column.visible, preferences::label(column.column));
                if ui.add_enabled(index > 0, egui::Button::new("↑")).clicked() {
                    move_column = Some((index, index.saturating_sub(1)));
                }
                if ui
                    .add_enabled(index + 1 < count, egui::Button::new("↓"))
                    .clicked()
                {
                    move_column = Some((index, index + 1));
                }
            });
        }
        if ui.button("Reset table defaults").clicked() {
            reset = true;
        }
    });
    if let Some((from, to)) = move_column {
        preferences.columns.swap(from, to);
    }
    if reset {
        *preferences = ContactTablePreferences::default();
    }
    let Some(page) = page else {
        ui.label("Contacts are not loaded.");
        return action;
    };
    if page.rows.is_empty() {
        ui.label("No contacts in this view.");
        return action;
    }
    if pending_focus.is_some_and(|id| !page.rows.iter().any(|row| row.qso.id == id)) {
        *pending_focus = None;
    }
    let visible: Vec<_> = preferences
        .columns
        .iter()
        .filter(|v| v.visible)
        .cloned()
        .collect();
    if visible.is_empty() {
        ui.label("Choose one or more columns to display.");
        return action;
    }
    let visible_order = visible.iter().map(|v| v.column).collect::<Vec<_>>();
    reset |= preferences.visible_order != visible_order;
    preferences.visible_order = visible_order;
    egui::ScrollArea::horizontal()
        .id_salt("contacts.horizontal")
        .show(ui, |ui| {
            let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 8.0;
            let max_height = (ui.available_height() - 65.0).max(60.0);
            let mut table = TableBuilder::new(ui)
                .id_salt("contacts.working")
                .max_scroll_height(max_height)
                .striped(true)
                .resizable(true)
                .sense(egui::Sense::click());
            for column in &visible {
                table = table.column(Column::initial(column.width).at_least(48.0));
            }
            if reset {
                table.reset();
            }
            if let Some(index) =
                pending_focus.and_then(|id| page.rows.iter().position(|row| row.qso.id == id))
            {
                table = table.scroll_to_row(index, Some(egui::Align::Center));
            }
            table
                .header(row_height, |mut header| {
                    for column in &visible {
                        header.col(|ui| {
                            if ui.button(preferences::label(column.column)).clicked() {
                                if preferences.order.column == column.column {
                                    preferences.order.direction = match preferences.order.direction
                                    {
                                        SortDirection::Ascending => SortDirection::Descending,
                                        SortDirection::Descending => SortDirection::Ascending,
                                    };
                                } else {
                                    preferences.order.column = column.column;
                                    preferences.order.direction = SortDirection::Ascending;
                                }
                            }
                        });
                    }
                })
                .body(|body| {
                    for (column, width) in visible.iter().zip(body.widths()) {
                        if let Some(saved) = preferences
                            .columns
                            .iter_mut()
                            .find(|v| v.column == column.column)
                        {
                            saved.width = *width;
                        }
                    }
                    body.rows(row_height, page.rows.len(), |mut row| {
                        let Some(value) = page.rows.get(row.index()) else {
                            return;
                        };
                        row.set_selected(selected.contains(&value.qso.id));
                        let mut has_focus = false;
                        for column in &visible {
                            let (_, cell_response) = row.col(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(cell(value, column.column)).monospace(),
                                    )
                                    .selectable(false),
                                );
                            });
                            has_focus |= cell_response.has_focus();
                        }
                        let response = row.response();
                        row_ids.insert(value.qso.id, response.id);
                        if response.clicked() {
                            *focused = Some(value.qso.id);
                            response.request_focus();
                            if response.ctx.input(|i| i.modifiers.command) {
                                if !selected.insert(value.qso.id) {
                                    selected.remove(&value.qso.id);
                                }
                            } else {
                                selected.clear();
                                selected.insert(value.qso.id);
                            }
                        }
                        if response.double_clicked() {
                            action.open = Some(value.qso.id);
                        }
                        if has_focus {
                            native_focus = Some(value.qso.id);
                        }
                        if *focused == Some(value.qso.id)
                            && response.ctx.memory(|m| m.focused()).is_none()
                        {
                            response.request_focus();
                        }
                    });
                });
        });
    if pending_focus.is_none()
        && let Some(id) = native_focus
    {
        *focused = Some(id);
    }
    if native_focus.is_some() || pending_focus.is_some() {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            action.open = *focused;
        }
        let delta = if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
        {
            1
        } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
            -1
        } else {
            0
        };
        if delta != 0 {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        if delta != 0
            && let Some(index) =
                focused.and_then(|id| page.rows.iter().position(|row| row.qso.id == id))
            && let Some(next) = index
                .checked_add_signed(delta)
                .and_then(|index| page.rows.get(index))
        {
            *focused = Some(next.qso.id);
            *pending_focus = Some(next.qso.id);
            selected.clear();
            selected.insert(next.qso.id);
            ui.ctx().request_repaint();
        }
    }
    if let Some(target) = pending_focus.and_then(|id| row_ids.get(&id)) {
        ui.memory_mut(|m| m.request_focus(*target));
        *pending_focus = None;
    }
    if pending_focus.is_some() {
        ui.ctx().request_repaint();
    }
    action
}

/// Format only the displayed native value; numeric/time ordering remains relational.
fn cell(row: &clacks_client_store::read::history::HistoryRow, column: ContactColumn) -> String {
    let qso = &row.qso;
    match column {
        ContactColumn::Callsign => qso.contacted_station.callsign.as_str().to_owned(),
        ContactColumn::Start => crate::logging::time::format(qso.start),
        ContactColumn::Band => qso
            .primary
            .band()
            .map(|v| v.label().to_owned())
            .unwrap_or_default(),
        ContactColumn::Mode => match qso.mode.identity() {
            ModeIdentity::Parent { label } | ModeIdentity::Specialization { label, .. } => label,
        },
        ContactColumn::Frequency => qso
            .primary
            .frequency()
            .map(|v| format!("{}.{:06}", v.get() / 1_000_000, v.get() % 1_000_000))
            .unwrap_or_default(),
        ContactColumn::Sent => qso.sent_report.clone().unwrap_or_default(),
        ContactColumn::Received => qso.received_report.clone().unwrap_or_default(),
        ContactColumn::Grid => qso
            .contacted_station
            .location
            .as_ref()
            .and_then(|v| v.position.as_ref())
            .map(|v| v.display_locator().to_string())
            .unwrap_or_default(),
        ContactColumn::Status => {
            if row.review.is_some() {
                "Conflict".into()
            } else if row.pending {
                "Pending".into()
            } else {
                String::new()
            }
        }
    }
}
