//! Prefix history for the shared unfinished contacted callsign.

use super::{
    ReadSnapshot,
    contacts::{ContactOrder, ContactPage, ContactPageQuery},
};
use crate::logbooks::selection::SelectedView;
use clacks_model::query::{
    Predicate, Query,
    field::Field,
    value::{TextMatch, Value},
};

/// Bounded Worked Before input independent of the history expression and selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkedBeforeQuery {
    /// Current selected contact view.
    pub view: SelectedView,
    /// Incomplete contacted callsign, normalized without discarding slash components.
    pub call_prefix: String,
    /// Native full-value ordering with stable contact-ID ties.
    pub order: ContactOrder,
    /// Matching contacts preceding the requested page.
    pub offset: u32,
    /// Maximum page size, from 1 through 1000.
    pub limit: u32,
}

impl Default for WorkedBeforeQuery {
    fn default() -> Self {
        Self {
            view: SelectedView::AllContacts,
            call_prefix: String::new(),
            order: Default::default(),
            offset: 0,
            limit: 100,
        }
    }
}

impl ReadSnapshot {
    /// Read prior saved working contacts for a literal callsign prefix.
    ///
    /// Empty input produces no contacts. No history expression, lookup provider or
    /// own callsign restricts this list; the selected view still applies.
    #[tracing::instrument(skip_all, name = "client_store.worked_before", level = "debug", fields(account_id = %self.account, limit = query.limit, offset = query.offset))]
    pub async fn worked_before(
        &mut self,
        query: &WorkedBeforeQuery,
    ) -> anyhow::Result<ContactPage> {
        let prefix = query.call_prefix.trim().to_uppercase();
        if prefix.is_empty() {
            return Ok(ContactPage {
                rows: vec![],
                matching_total: 0,
                selected_total: 0,
                selected_outside_filter: 0,
                has_more: false,
            });
        }
        self.contact_page(&ContactPageQuery {
            view: query.view,
            filter: Query::Predicate(Predicate {
                field: Field::ContactCall,
                value: Value::Text {
                    text: prefix,
                    matching: TextMatch::Prefix,
                },
            }),
            order: query.order,
            offset: query.offset,
            limit: query.limit,
            ..Default::default()
        })
        .await
    }
}
