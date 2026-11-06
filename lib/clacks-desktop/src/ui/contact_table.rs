//! Continuous virtual history with bounded reads and native identity selection.

pub(super) mod data;
mod header;
pub(super) mod preferences;
#[cfg(test)]
mod tests;

use clacks_client_store::read::contacts::ContactColumn;
use clacks_model::ids::QsoId;
use clacks_model::recognition::mode::ModeIdentity;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use preferences::ContactTablePreferences;
use std::collections::BTreeSet;

/// Accepted rows and optional loading feedback within the table viewport.
pub(super) struct TableContent<'a> {
    /// Loaded viewport pages and the complete logical result count.
    pub data: &'a data::TableData,
    /// Display the selected set rather than matches of the expression.
    pub selected_only: bool,
    /// Reset the viewport after a changed query or ordering.
    pub reset_scroll: bool,
    /// Loading feedback supplied by the query owner when no page is available.
    pub loading_message: Option<&'a str>,
}

/// Row activation and visible data demand from one table presentation.
#[derive(Default)]
pub(super) struct TableAction {
    /// Stable identity to load, independent of row order.
    pub open: Option<QsoId>,
    /// Internal page offsets needed by visible rows and a pending keyboard target.
    pub needed: BTreeSet<u32>,
}

/// Keyboard navigation intent retained while its destination row is loading.
#[derive(Clone, Copy)]
pub(super) struct PendingFocus {
    /// Absolute row to reveal and focus when available.
    pub index: usize,
    /// Native focus owner that initiated navigation; a new owner cancels it.
    pub owner: Option<egui::Id>,
}

/// Minimum viewport containing column headings, one complete row and the
/// horizontal scrollbar without covering that row.
pub(super) fn minimum_height(ui: &egui::Ui) -> f32 {
    2.0 * row_height(ui)
        + ui.spacing().item_spacing.y
        + ui.spacing()
            .scroll
            .allocated_width()
            .max(ui.spacing().scroll.bar_width)
}

/// Shared height for table headings and data rows at the current typography.
fn row_height(ui: &egui::Ui) -> f32 {
    (ui.text_style_height(&egui::TextStyle::Monospace) + 8.0).max(ui.spacing().interact_size.y)
}

/// Display current rows; sorting requests change the SQL query rather than sorting payloads.
pub(super) fn show(
    ui: &mut egui::Ui,
    content: TableContent<'_>,
    preferences: &mut ContactTablePreferences,
    selected: &mut BTreeSet<QsoId>,
    focused: &mut Option<QsoId>,
    pending_focus: &mut Option<PendingFocus>,
    today: Option<i64>,
) -> TableAction {
    let mut action = TableAction::default();
    let mut row_ids = std::collections::BTreeMap::new();
    let mut native_focus = None;
    let data = content.data;
    let total = data.len(content.selected_only);
    let navigation_interrupted = ui.input(|input| {
        input.pointer.any_pressed()
            || input.key_pressed(egui::Key::Tab)
            || input.key_pressed(egui::Key::Escape)
    });
    if content.reset_scroll
        || navigation_interrupted
        || pending_focus.is_some_and(|target| {
            target.index >= total
                || ui.memory(|memory| memory.focused().is_some_and(|id| Some(id) != target.owner))
        })
    {
        *pending_focus = None;
    }
    if data.counts.is_none() {
        action.needed.insert(0);
    }
    let visible: Vec<_> = preferences
        .columns
        .iter()
        .filter(|v| v.visible)
        .cloned()
        .collect();
    if visible.is_empty() {
        action.needed.insert(0);
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
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let row_height = row_height(ui);
            let reserved = row_height + ui.spacing().item_spacing.y;
            let max_height = (ui.available_height() - reserved).max(0.0);
            let mut table = TableBuilder::new(ui)
                .id_salt("contacts.working")
                .max_scroll_height(max_height)
                .min_scrolled_height(max_height)
                .striped(true)
                .resizable(true)
                .sense(egui::Sense::click());
            for column in &visible {
                table = table.column(Column::initial(column.width).at_least(48.0));
            }
            if reset {
                table.reset();
            }
            if content.reset_scroll {
                table = table.vertical_scroll_offset(0.0);
            } else if let Some(target) = *pending_focus {
                table = table.scroll_to_row(target.index, Some(egui::Align::Center));
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
                .body(|mut body| {
                    for (column, width) in visible.iter().zip(body.widths()) {
                        if let Some(saved) = preferences
                            .columns
                            .iter_mut()
                            .find(|v| v.column == column.column)
                        {
                            saved.width = *width;
                        }
                    }
                    if data.counts.is_none() {
                        if let Some(message) = content.loading_message {
                            body.ui_mut().label(message);
                        }
                        return;
                    }
                    if total == 0 {
                        body.ui_mut().label("No contacts in this view.");
                        return;
                    }
                    body.rows(row_height, total, |mut row| {
                        let index = row.index();
                        if let Some(offset) = data::page_offset(index) {
                            action.needed.insert(offset);
                        }
                        let Some(value) = data.row(index) else {
                            for (column, _) in visible.iter().enumerate() {
                                row.col(|ui| {
                                    if column == 0 {
                                        ui.weak(
                                            content
                                                .loading_message
                                                .unwrap_or("Contact unavailable"),
                                        );
                                    }
                                });
                            }
                            return;
                        };
                        row.set_selected(selected.contains(&value.working.qso.id()));
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
                        row_ids.insert(index, response.id);
                        if response.clicked() {
                            *focused = Some(value.working.qso.id());
                            response.request_focus();
                            if response.ctx.input(|i| i.modifiers.command) {
                                if !selected.insert(value.working.qso.id()) {
                                    selected.remove(&value.working.qso.id());
                                }
                            } else {
                                selected.clear();
                                selected.insert(value.working.qso.id());
                            }
                        }
                        if response.double_clicked() {
                            action.open = Some(value.working.qso.id());
                        }
                        if has_focus {
                            native_focus = Some(value.working.qso.id());
                        }
                        if *focused == Some(value.working.qso.id())
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
    if pending_focus.is_none()
        && let Some(id) = native_focus
    {
        *focused = Some(id);
    }
    if native_focus.is_some() || pending_focus.is_some() {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            && pending_focus.is_none()
        {
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
            && let Some(index) = pending_focus
                .map(|target| target.index)
                .or_else(|| focused.and_then(|id| data.index(id)))
            && let Some(next) = index
                .checked_add_signed(delta)
                .filter(|index| *index < total)
        {
            let owner = pending_focus.map_or_else(
                || ui.memory(|memory| memory.focused()),
                |target| target.owner,
            );
            *pending_focus = Some(PendingFocus { index: next, owner });
            ui.ctx().request_repaint();
        }
    }
    if let Some(offset) = pending_focus.and_then(|target| data::page_offset(target.index)) {
        action.needed.insert(offset);
    }
    if let Some(pending) = *pending_focus
        && let Some(value) = data.row(pending.index)
        && let Some(target) = row_ids.get(&pending.index)
    {
        *focused = Some(value.working.qso.id());
        selected.clear();
        selected.insert(value.working.qso.id());
        ui.memory_mut(|m| m.request_focus(*target));
        *pending_focus = None;
    }
    if pending_focus
        .and_then(|target| data.row(target.index))
        .is_some()
    {
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
        ContactColumn::Callsign => qso.contacted_station().callsign.as_str().to_owned(),
        ContactColumn::Start => crate::logging::time::format(qso.start()),
        ContactColumn::End => qso
            .end()
            .map(crate::logging::time::format)
            .unwrap_or_default(),
        ContactColumn::OwnCall => qso.own_station().callsign.as_str().to_owned(),
        ContactColumn::OwnGrid => row
            .facts
            .own_location
            .as_ref()
            .and_then(|s| s.value.location.as_ref())
            .and_then(|l| l.position().as_ref())
            .map(|p| p.display_locator().to_string())
            .unwrap_or_default(),
        ContactColumn::OwnReferences => {
            references(&qso.own_station().references, &row.facts.group_names)
        }
        ContactColumn::ContactReferences => {
            references(&qso.contacted_station().references, &row.facts.group_names)
        }
        ContactColumn::Band => qso
            .primary()
            .band()
            .map(|v| v.label().to_owned())
            .unwrap_or_default(),
        ContactColumn::Mode => match qso.mode().identity() {
            ModeIdentity::Parent { label } | ModeIdentity::Specialization { label, .. } => label,
        },
        ContactColumn::Frequency => qso
            .primary()
            .frequency()
            .map(|v| format!("{}.{:06}", v.get() / 1_000_000, v.get() % 1_000_000))
            .unwrap_or_default(),
        ContactColumn::Sent => qso.sent_report().clone().unwrap_or_default(),
        ContactColumn::Received => qso.received_report().clone().unwrap_or_default(),
        ContactColumn::Grid => row
            .facts
            .contact_location
            .as_ref()
            .and_then(|s| s.value.location.as_ref())
            .and_then(|v| v.position().as_ref())
            .map(|v| v.display_locator().to_string())
            .unwrap_or_default(),
        ContactColumn::Status => {
            let mut markers = Vec::new();
            if today.is_some_and(|now| qso.start().unix_seconds().div_euclid(86_400) == now) {
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
            r.identifier()
                .as_ref()
                .map_or_else(|| group.clone(), |id| format!("{group}/{id}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}
