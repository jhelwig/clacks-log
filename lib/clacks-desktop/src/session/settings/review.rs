//! Account-wide review reads on the existing workspace runtime.

use clacks_client_store::{
    read::{
        self,
        review::{ReviewPage, ReviewQuery, ReviewRow},
    },
    settings::SettingsSnapshot,
    store::ClientStore,
};
use clacks_sync::record::RecordKey;

/// Owned review operation; the caller retains its response receiver.
pub(crate) enum ReviewAction {
    /// Read a bounded account-wide section with full filtered counts.
    ReadPage {
        /// Exact section, type and continuation requested by its originating view.
        query: ReviewQuery,
    },
    /// Read exact roles and their editor catalogs consistently.
    ReadRecord {
        /// Account-qualified target; never inferred from the current UI selection.
        key: RecordKey,
    },
}

/// A committed snapshot read qualified by its original selection.
#[derive(Debug)]
pub(crate) enum ReviewResponse {
    /// Bounded section selected by the accompanying query.
    Page {
        /// Exact selection used by the query.
        query: ReviewQuery,
        /// Rows and counts from the completed read snapshot.
        page: ReviewPage,
        /// Catalog used to disambiguate names in the same snapshot.
        settings: Box<SettingsSnapshot>,
    },
    /// Exact record, even when its pending work has just settled.
    Record {
        /// Requested account-qualified identity.
        key: RecordKey,
        /// Complete roles, absent only when the record is unavailable locally.
        row: Option<Box<ReviewRow>>,
        /// Current editor definitions and source catalogs.
        settings: Box<SettingsSnapshot>,
    },
}

/// Execute reads without retaining database snapshots while the operator decides.
pub(super) async fn execute(
    store: &ClientStore,
    action: ReviewAction,
) -> anyhow::Result<ReviewResponse> {
    let mut snapshot = read::snapshot(store).await?;
    let result = match action {
        ReviewAction::ReadPage { query } => {
            let page = snapshot.review_page(&query).await?;
            ReviewResponse::Page {
                query,
                page,
                settings: Box::new(snapshot.settings().await?),
            }
        }
        ReviewAction::ReadRecord { key } => ReviewResponse::Record {
            key,
            row: snapshot.review_record(key).await?.map(Box::new),
            settings: Box::new(snapshot.settings().await?),
        },
    };
    snapshot.close().await?;
    Ok(result)
}
