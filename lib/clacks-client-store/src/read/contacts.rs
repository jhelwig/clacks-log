//! Bounded selected-view pages for the desktop contact table.

use super::{history::HistoryRow, projection::ContactFacts};
use crate::logbooks::selection::SelectedView;
use clacks_model::{ids::QsoId, query::Query};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Native value used by a contact table column.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ContactColumn {
    /// Contacted callsign.
    Callsign,
    /// Full UTC start.
    Start,
    /// Primary band label.
    Band,
    /// Effective mode label.
    Mode,
    /// Primary whole-hertz frequency.
    Frequency,
    /// Sent report text.
    Sent,
    /// Received report text.
    Received,
    /// Recorded contacted-station grid area.
    Grid,
    /// Pending/conflict status.
    Status,
    /// Full UTC end, when recorded.
    End,
    /// Recorded own station callsign.
    OwnCall,
    /// Effective own station grid.
    OwnGrid,
    /// Recorded own participation and references.
    OwnReferences,
    /// Recorded contacted participation and references.
    ContactReferences,
}

/// Direction for present values, with absent values always last.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum SortDirection {
    /// Smaller values first.
    Ascending,
    /// Larger values first.
    Descending,
}

/// One column order with stable contact-ID ties.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContactOrder {
    /// Recorded comparison value.
    pub column: ContactColumn,
    /// Order for present values.
    pub direction: SortDirection,
}

impl Default for ContactOrder {
    fn default() -> Self {
        Self {
            column: ContactColumn::Start,
            direction: SortDirection::Descending,
        }
    }
}

/// Bounded selection in a local contact view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactPageQuery {
    /// Selected tag or built-in view.
    pub view: SelectedView,
    /// Validated expression applied independently of view and selection.
    pub filter: Query,
    /// Stable selected identities, including contacts off this page.
    pub selected_ids: BTreeSet<QsoId>,
    /// Inspect selected active contacts instead of restricting rows by expression.
    pub show_selected: bool,
    /// Native ordering and deterministic ties.
    pub order: ContactOrder,
    /// Rows preceding this page.
    pub offset: u32,
    /// Page size, from 1 through 1000.
    pub limit: u32,
}

impl Default for ContactPageQuery {
    fn default() -> Self {
        Self {
            view: SelectedView::AllContacts,
            filter: Query::All,
            selected_ids: BTreeSet::new(),
            show_selected: false,
            order: Default::default(),
            offset: 0,
            limit: 100,
        }
    }
}

/// Local working rows and whether another bounded page exists.
#[derive(Debug)]
pub struct ContactPage {
    /// Unique working contacts with pending/review metadata.
    pub rows: Vec<ContactRow>,
    /// All matches in the applied view/expression, independent of paging.
    pub matching_total: u64,
    /// Active selected identities within the current view.
    pub selected_total: u64,
    /// Selected identities outside the expression, not merely off this page.
    pub selected_outside_filter: u64,
    /// Another page follows; not an account-wide count.
    pub has_more: bool,
}

/// One saved working contact with separate effective read information.
#[derive(Clone, Debug)]
pub struct ContactRow {
    /// Saved native values and pending/conflict context.
    pub working: HistoryRow,
    /// Native-selected fallback information and service evidence.
    pub facts: ContactFacts,
}
