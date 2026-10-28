//! Local contact-table presentation, independent of saved contacts and selected view.

use clacks_client_store::read::contacts::{ContactColumn, ContactOrder};
use serde::{Deserialize, Serialize};

/// One selectable, movable contact column and its last width.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ColumnPreference {
    /// Native recorded value displayed.
    pub column: ContactColumn,
    /// Display the column without changing its data.
    pub visible: bool,
    /// Width in UI points.
    pub width: f32,
}

/// Independent history-table preferences persisted through eframe.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct ContactTablePreferences {
    /// All available columns in configured order.
    pub columns: Vec<ColumnPreference>,
    /// Last chosen native sorting.
    pub order: ContactOrder,
    /// Last visible column identities used by the toolkit's positional size cache.
    #[serde(skip)]
    pub visible_order: Vec<ContactColumn>,
}

impl Default for ContactTablePreferences {
    fn default() -> Self {
        use ContactColumn::*;
        Self {
            columns: [
                Callsign,
                Start,
                Band,
                Mode,
                Frequency,
                Sent,
                Received,
                Grid,
                Status,
                End,
                OwnCall,
                OwnGrid,
                OwnReferences,
                ContactReferences,
            ]
            .into_iter()
            .map(|column| ColumnPreference {
                column,
                visible: !matches!(
                    column,
                    End | OwnCall | OwnGrid | OwnReferences | ContactReferences
                ),
                width: if column == Start { 180.0 } else { 90.0 },
            })
            .collect(),
            order: Default::default(),
            visible_order: vec![],
        }
    }
}

impl ContactTablePreferences {
    /// Restore usable widths and complete column availability without affecting account facts.
    pub fn restore(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(mut value) =
            storage.and_then(|s| eframe::get_value::<Self>(s, "clacks.contacts.table.v1"))
        else {
            return Self::default();
        };
        let defaults = Self::default();
        let mut seen = Vec::new();
        value.columns.retain(|v| {
            if seen.contains(&v.column) {
                false
            } else {
                seen.push(v.column);
                true
            }
        });
        for column in defaults.columns {
            if !seen.contains(&column.column) {
                value.columns.push(column);
            }
        }
        for column in &mut value.columns {
            if !column.width.is_finite() {
                column.width = 90.0;
            }
            column.width = column.width.clamp(48.0, 600.0);
        }
        value
    }

    /// Save only table presentation, never selection or editable facts.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "clacks.contacts.table.v1", self);
    }
}

/// Human-facing title for a recorded value column.
pub(super) fn label(column: ContactColumn) -> &'static str {
    use ContactColumn::*;
    match column {
        Callsign => "Callsign",
        Start => "Start · UTC",
        End => "End · UTC",
        OwnCall => "Own call",
        OwnGrid => "Own grid",
        OwnReferences => "Own references",
        ContactReferences => "Contact references",
        Band => "Band",
        Mode => "Mode",
        Frequency => "MHz",
        Sent => "Sent",
        Received => "Received",
        Grid => "DX grid",
        Status => "Status",
    }
}
