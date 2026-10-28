//! Bounded selected-view pages for the desktop contact table.

use super::{ReadSnapshot, history::HistoryRow};
use crate::{logbooks::selection::SelectedView, rows::contact::WorkingContactRow};
use serde::{Deserialize, Serialize};

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
    /// Optional explicit selected-ID subset within this view.
    pub selected: Option<std::collections::BTreeSet<clacks_model::ids::QsoId>>,
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
            selected: None,
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
    pub rows: Vec<HistoryRow>,
    /// Another page follows; not an account-wide count.
    pub has_more: bool,
}

impl ReadSnapshot {
    /// Read one selected-view page without loading or sorting the entire account in memory.
    #[tracing::instrument(skip_all, name = "client_store.contacts.page", level = "debug", fields(account_id = %self.account, limit = query.limit, offset = query.offset))]
    pub async fn contact_page(&mut self, query: &ContactPageQuery) -> anyhow::Result<ContactPage> {
        anyhow::ensure!(
            (1..=1000).contains(&query.limit),
            "Contact page size must be between 1 and 1000"
        );
        let account = self.account.to_string();
        let tag = match query.view {
            SelectedView::Logbook(id) => Some(id.to_string()),
            _ => None,
        };
        let untagged = matches!(query.view, SelectedView::Untagged);
        let column = match query.order.column {
            ContactColumn::Callsign => "call",
            ContactColumn::Start => "start",
            ContactColumn::Band => "band",
            ContactColumn::Mode => "mode",
            ContactColumn::Frequency => "frequency",
            ContactColumn::Sent => "sent",
            ContactColumn::Received => "received",
            ContactColumn::Grid => "grid",
            ContactColumn::Status => "status",
        };
        let descending = matches!(query.order.direction, SortDirection::Descending);
        let limit = i64::from(query.limit) + 1;
        let offset = i64::from(query.offset);
        let selected = query.selected.as_ref().map(|ids| {
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        });
        let mut rows = sqlx::query_file_as!(
            WorkingContactRow,
            "sql/read/contacts.sql",
            account,
            tag,
            untagged,
            column,
            descending,
            limit,
            offset,
            selected
        )
        .fetch_all(&mut *self.transaction)
        .await?;
        let has_more = rows.len() > usize::try_from(query.limit)?;
        rows.truncate(usize::try_from(query.limit)?);
        Ok(ContactPage {
            rows: super::history::working_contacts(&mut self.transaction, self.account, rows)
                .await?,
            has_more,
        })
    }
}
