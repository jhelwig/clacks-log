//! Full-cell table headings and their column-configuration context menu.

use super::preferences::{self, ContactTablePreferences};
use crate::ui::theme;
use clacks_client_store::read::contacts::{ContactColumn, SortDirection};
use eframe::egui;

/// A header interaction applied after the current table finishes reporting widths.
pub(super) enum HeaderAction {
    /// Toggle the clicked column's sorting direction or select ascending order.
    Sort(ContactColumn),
    /// Include or hide a column without discarding its saved width.
    Visibility {
        /// The column whose inclusion changes.
        column: ContactColumn,
        /// Whether the column belongs in the displayed table.
        visible: bool,
    },
    /// Move the contextual column one position toward the beginning.
    MoveLeft(ContactColumn),
    /// Move the contextual column one position toward the end.
    MoveRight(ContactColumn),
    /// Restore this table's accepted initial columns, widths and sorting.
    Reset,
}

impl HeaderAction {
    /// Update canonical preferences after the rendered layout has been retained.
    pub(super) fn apply(self, preferences: &mut ContactTablePreferences) {
        match self {
            Self::Sort(column) => {
                preferences.order.direction = if preferences.order.column == column {
                    match preferences.order.direction {
                        SortDirection::Ascending => SortDirection::Descending,
                        SortDirection::Descending => SortDirection::Ascending,
                    }
                } else {
                    SortDirection::Ascending
                };
                preferences.order.column = column;
            }
            Self::Visibility { column, visible } => {
                if let Some(value) = preferences.columns.iter_mut().find(|c| c.column == column) {
                    value.visible = visible;
                }
            }
            Self::MoveLeft(column) | Self::MoveRight(column) => {
                let delta = if matches!(self, Self::MoveLeft(_)) {
                    -1
                } else {
                    1
                };
                if let Some(index) = preferences.columns.iter().position(|c| c.column == column)
                    && let Some(target) = index.checked_add_signed(delta)
                    && target < preferences.columns.len()
                {
                    preferences.columns.swap(index, target);
                }
            }
            Self::Reset => *preferences = ContactTablePreferences::defaults(preferences.kind),
        }
    }
}

/// Paint a continuous header cell, retaining native focus, sorting and context menus.
///
/// An absent column supplies a recovery target for a layout with all columns hidden.
/// Configuration remains available regardless of the table's current result rows.
pub(super) fn show(
    ui: &mut egui::Ui,
    column: Option<ContactColumn>,
    preferences: &ContactTablePreferences,
) -> Option<HeaderAction> {
    let label = column.map_or("Right-click to choose columns", preferences::label);
    let colors = theme::palette(ui.ctx().theme());
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ColumnHeader, ui.is_enabled(), label)
    });

    ui.painter().rect_filled(
        rect,
        0,
        if response.hovered() {
            colors.hover
        } else {
            colors.header
        },
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        egui::Stroke::new(1.0, colors.border),
    );
    ui.painter().vline(
        rect.right(),
        rect.y_range(),
        egui::Stroke::new(1.0, colors.border),
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            0,
            egui::Stroke::new(1.0, colors.accent),
            egui::StrokeKind::Inside,
        );
    }

    let sorted = column.is_some_and(|column| preferences.order.column == column);
    let padding = ui.spacing().button_padding.x;
    let indicator_width = if sorted {
        ui.spacing().interact_size.y
    } else {
        0.0
    };
    let galley = egui::WidgetText::from(label).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (rect.width() - 2.0 * padding - indicator_width).max(0.0),
        egui::TextStyle::Body,
    );
    ui.painter().galley(
        egui::pos2(
            rect.left() + padding,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        colors.text,
    );

    if sorted {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_sort_direction(match preferences.order.direction {
                SortDirection::Ascending => egui::accesskit::SortDirection::Ascending,
                SortDirection::Descending => egui::accesskit::SortDirection::Descending,
            });
        });
        let direction = match preferences.order.direction {
            SortDirection::Ascending => -1.0,
            SortDirection::Descending => 1.0,
        };
        let half = ui.text_style_height(&egui::TextStyle::Small) / 4.0;
        let center = egui::pos2(rect.right() - padding - half, rect.center().y);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(0.0, direction * half),
                center + egui::vec2(-half, -direction * half),
                center + egui::vec2(half, -direction * half),
            ],
            colors.muted,
            egui::Stroke::NONE,
        ));
    }

    let keyboard_sort = response.has_focus()
        && ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
        });
    let mut action = column
        .filter(|_| response.clicked() || keyboard_sort)
        .map(HeaderAction::Sort);
    response.context_menu(|ui| {
        action = menu(ui, column, preferences).or(action.take());
    });
    action
}

/// Offer the shared visibility choices and contextual ordering/reset actions.
fn menu(
    ui: &mut egui::Ui,
    column: Option<ContactColumn>,
    preferences: &ContactTablePreferences,
) -> Option<HeaderAction> {
    let mut action = None;
    ui.strong("Visible columns");
    for value in &preferences.columns {
        let mut visible = value.visible;
        if ui
            .checkbox(&mut visible, preferences::label(value.column))
            .changed()
        {
            action = Some(HeaderAction::Visibility {
                column: value.column,
                visible,
            });
        }
    }

    ui.separator();
    if let Some(column) = column
        && let Some(index) = preferences.columns.iter().position(|c| c.column == column)
    {
        if ui
            .add_enabled(index > 0, egui::Button::new("Move column left"))
            .clicked()
        {
            action = Some(HeaderAction::MoveLeft(column));
            ui.close();
        }
        if ui
            .add_enabled(
                index + 1 < preferences.columns.len(),
                egui::Button::new("Move column right"),
            )
            .clicked()
        {
            action = Some(HeaderAction::MoveRight(column));
            ui.close();
        }
        ui.separator();
    }
    if ui.button("Reset table defaults").clicked() {
        action = Some(HeaderAction::Reset);
        ui.close();
    }
    action
}
