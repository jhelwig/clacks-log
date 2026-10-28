//! Real bounded history rows with native IDs, selection and column presentation.

mod header;
pub(super) mod preferences;
#[cfg(test)]
mod tests;

use clacks_client_store::read::contacts::ContactColumn;
use clacks_client_store::read::contacts::ContactPage;
use clacks_model::ids::QsoId;
use clacks_model::recognition::mode::ModeIdentity;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use preferences::ContactTablePreferences;
use std::collections::BTreeSet;

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
    today: Option<i64>,
) -> TableAction {
    let mut action = TableAction::default();
    let mut row_ids = std::collections::BTreeMap::new();
    let mut native_focus = None;

    let rows = page.map_or(&[][..], |page| page.rows.as_slice());
    if pending_focus.is_some_and(|id| !rows.iter().any(|row| row.working.qso.id == id)) {
        *pending_focus = None;
    }
    let visible: Vec<_> = preferences
        .columns
        .iter()
        .filter(|v| v.visible)
        .cloned()
        .collect();
    if visible.is_empty() {
        let height = ui.spacing().interact_size.y;
        let choice = ui
            .allocate_ui(egui::vec2(ui.available_width(), height), |ui| {
                header::show(ui, None, preferences)
            })
            .inner;
        if let Some(choice) = choice {
            choice.apply(preferences);
            ui.ctx().request_repaint();
        }
        return action;
    }
    let layout_id = ui.make_persistent_id("contact_table.applied_columns");
    let applied = ui.data(|data| data.get_temp::<Vec<preferences::ColumnPreference>>(layout_id));
    let reset = applied.as_ref() != Some(&visible);
    let mut header_action = None;
    egui::ScrollArea::horizontal()
        .id_salt("contacts.horizontal")
        .show(ui, |ui| {
            let row_height = (ui.text_style_height(&egui::TextStyle::Monospace) + 8.0)
                .max(ui.spacing().interact_size.y);
            let reserved = row_height
                + ui.spacing().interact_size.y
                + ui.spacing().item_spacing.y * 4.0
                + ui.spacing().scroll.bar_width;
            let max_height = (ui.available_height() - reserved).max(0.0);
            let mut table = TableBuilder::new(ui)
                .id_salt("contacts.working")
                .max_scroll_height(max_height)
                .min_scrolled_height(0.0)
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
                pending_focus.and_then(|id| rows.iter().position(|row| row.working.qso.id == id))
            {
                table = table.scroll_to_row(index, Some(egui::Align::Center));
            }
            table
                .header(row_height, |mut header| {
                    for column in &visible {
                        header.col(|ui| {
                            if let Some(choice) = header::show(ui, Some(column.column), preferences)
                            {
                                header_action = Some(choice);
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
                    body.rows(row_height, rows.len(), |mut row| {
                        let Some(value) = rows.get(row.index()) else {
                            return;
                        };
                        row.set_selected(selected.contains(&value.working.qso.id));
                        let mut has_focus = false;
                        for column in &visible {
                            let (_, cell_response) = row.col(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(cell(value, column.column, today))
                                            .monospace(),
                                    )
                                    .truncate()
                                    .selectable(false),
                                );
                            });
                            has_focus |= cell_response.has_focus();
                        }
                        let response = row.response();
                        row_ids.insert(value.working.qso.id, response.id);
                        if response.clicked() {
                            *focused = Some(value.working.qso.id);
                            response.request_focus();
                            if response.ctx.input(|i| i.modifiers.command) {
                                if !selected.insert(value.working.qso.id) {
                                    selected.remove(&value.working.qso.id);
                                }
                            } else {
                                selected.clear();
                                selected.insert(value.working.qso.id);
                            }
                        }
                        if response.double_clicked() {
                            action.open = Some(value.working.qso.id);
                        }
                        if has_focus {
                            native_focus = Some(value.working.qso.id);
                        }
                        if *focused == Some(value.working.qso.id)
                            && response.ctx.memory(|m| m.focused()).is_none()
                        {
                            response.request_focus();
                        }
                    });
                });
        });
    // Each table has its own toolkit width cache. A change from the other
    // presentation must reset this cache before it can report widths back.
    let applied: Vec<_> = preferences
        .columns
        .iter()
        .filter(|column| column.visible)
        .cloned()
        .collect();
    ui.data_mut(|data| data.insert_temp(layout_id, applied));
    if let Some(choice) = header_action {
        choice.apply(preferences);
        ui.ctx().request_repaint();
    }
    if page.is_some() && rows.is_empty() {
        ui.label("No contacts in this view.");
    }
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
                focused.and_then(|id| rows.iter().position(|row| row.working.qso.id == id))
            && let Some(next) = index
                .checked_add_signed(delta)
                .and_then(|index| rows.get(index))
        {
            *focused = Some(next.working.qso.id);
            *pending_focus = Some(next.working.qso.id);
            selected.clear();
            selected.insert(next.working.qso.id);
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
fn cell(
    row: &clacks_client_store::read::contacts::ContactRow,
    column: ContactColumn,
    today: Option<i64>,
) -> String {
    let qso = &row.working.qso;
    match column {
        ContactColumn::Callsign => qso.contacted_station.callsign.as_str().to_owned(),
        ContactColumn::Start => crate::logging::time::format(qso.start),
        ContactColumn::End => qso
            .end
            .map(crate::logging::time::format)
            .unwrap_or_default(),
        ContactColumn::OwnCall => qso.own_station.callsign.as_str().to_owned(),
        ContactColumn::OwnGrid => row
            .facts
            .own_location
            .as_ref()
            .and_then(|s| s.value.as_ref())
            .and_then(|l| l.position.as_ref())
            .map(|p| p.display_locator().to_string())
            .unwrap_or_default(),
        ContactColumn::OwnReferences => {
            references(&qso.own_station.references, &row.facts.group_names)
        }
        ContactColumn::ContactReferences => {
            references(&qso.contacted_station.references, &row.facts.group_names)
        }
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
        ContactColumn::Grid => row
            .facts
            .contact_location
            .as_ref()
            .and_then(|s| s.value.as_ref())
            .and_then(|v| v.position.as_ref())
            .map(|v| v.display_locator().to_string())
            .unwrap_or_default(),
        ContactColumn::Status => {
            let mut markers = Vec::new();
            if today.is_some_and(|now| qso.start.unix_seconds().div_euclid(86_400) == now) {
                markers.push("Today");
            }
            if row.working.review.is_some() {
                markers.push("Conflict");
            } else if row.working.pending {
                markers.push("Pending");
            }
            markers.join(" · ")
        }
    }
}

/// Show recorded associations using current group names, retaining unknown identities.
fn references(
    values: &std::collections::BTreeSet<clacks_model::reference::ReferenceAssociation>,
    names: &std::collections::BTreeMap<clacks_model::ids::GroupId, String>,
) -> String {
    values
        .iter()
        .map(|r| {
            let group = names
                .get(&r.group_id)
                .cloned()
                .unwrap_or_else(|| r.group_id.to_string());
            r.identifier
                .as_ref()
                .map_or_else(|| group.clone(), |id| format!("{group}/{id}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}
