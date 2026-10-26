//! Offline named-tag management and workspace-local contact view selection.

pub mod deletion;
pub(crate) mod read;
pub mod selection;
pub mod tags;

use clacks_sync::client::ClientRecord;
use selection::SelectedView;

/// Saved view selection and logbook tag records from one database snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogbookSnapshot {
    /// Workspace-local view that supplies the default membership for new contacts.
    pub selected_view: SelectedView,
    /// Account-owned tag records, including pending edits, deletions and unavailable values.
    pub tags: Vec<ClientRecord>,
}
