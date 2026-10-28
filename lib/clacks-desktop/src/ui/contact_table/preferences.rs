//! Local contact-table presentation, independent of saved contacts and selected view.

use clacks_client_store::read::contacts::{ContactColumn, ContactOrder};
use serde::{Deserialize, Serialize};

/// Independent presentation preferences for the two actual contact-table uses.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::ui) enum TableKind {
    /// Main contact history.
    #[default]
    History,
    /// Prior contacts for the unfinished entry's callsign prefix.
    WorkedBefore,
}

impl TableKind {
    /// Stable local storage key, independent of logbook and viewport.
    fn key(self) -> &'static str {
        match self {
            Self::History => "clacks.contacts.table.v1",
            Self::WorkedBefore => "clacks.worked_before.table.v1",
        }
    }
}

/// One selectable, movable contact column and its last width.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(in crate::ui) struct ColumnPreference {
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
pub(in crate::ui) struct ContactTablePreferences {
    /// Actual owning table; both Worked Before presentations share one value.
    #[serde(skip)]
    pub kind: TableKind,
    /// All available columns in configured order.
    pub columns: Vec<ColumnPreference>,
    /// Last chosen native sorting.
    pub order: ContactOrder,
}

impl Default for ContactTablePreferences {
    fn default() -> Self {
        Self::defaults(TableKind::History)
    }
}

impl ContactTablePreferences {
    /// Choose the accepted initial columns without changing stored user choices.
    pub fn defaults(kind: TableKind) -> Self {
        use ContactColumn::*;
        let order = match kind {
            TableKind::History => vec![
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
            ],
            TableKind::WorkedBefore => vec![
                Start,
                Callsign,
                Band,
                Mode,
                Grid,
                ContactReferences,
                Status,
                Frequency,
                Sent,
                Received,
                End,
                OwnCall,
                OwnGrid,
                OwnReferences,
            ],
        };
        Self {
            kind,
            columns: order
                .into_iter()
                .map(|column| ColumnPreference {
                    column,
                    visible: match kind {
                        TableKind::History => !matches!(
                            column,
                            End | OwnCall | OwnGrid | OwnReferences | ContactReferences
                        ),
                        TableKind::WorkedBefore => matches!(
                            column,
                            Start | Callsign | Band | Mode | Grid | ContactReferences | Status
                        ),
                    },
                    width: if column == Start { 180.0 } else { 90.0 },
                })
                .collect(),
            order: Default::default(),
        }
    }
}

impl ContactTablePreferences {
    /// Restore usable widths and complete column availability without affecting account facts.
    pub fn restore(storage: Option<&dyn eframe::Storage>, kind: TableKind) -> Self {
        let Some(mut value) = storage.and_then(|s| eframe::get_value::<Self>(s, kind.key())) else {
            return Self::defaults(kind);
        };
        value.kind = kind;
        let defaults = Self::defaults(kind);
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
        eframe::set_value(storage, self.kind.key(), self);
    }
}

/// Human-facing title for a recorded value column.
pub(in crate::ui) fn label(column: ContactColumn) -> &'static str {
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
