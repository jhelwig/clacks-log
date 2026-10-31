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
    /// Save one explicit resolution against the exact shown roles.
    Resolve {
        /// Saved client record compared by the operator.
        expected: Box<clacks_sync::client::ClientRecord>,
        /// Exact original, local, shared and retained comparison.
        shown: Box<clacks_sync::client::ReviewInput>,
        /// Complete validated native result.
        chosen: clacks_sync::record::RecordState,
    },
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
    /// Authoritative guarded-save result for its originating record.
    Resolved {
        /// Native account-qualified target.
        key: RecordKey,
        /// Committed, unchanged or stale outcome, independent of view refresh.
        outcome: Box<clacks_client_store::save::SaveOutcome>,
    },
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
    match action {
        ReviewAction::Resolve {
            expected,
            shown,
            chosen,
        } => {
            let key = expected.key();
            let outcome =
                clacks_client_store::synchronization::resolve(store, &expected, &shown, chosen)
                    .await?;
            Ok(ReviewResponse::Resolved {
                key,
                outcome: Box::new(outcome),
            })
        }
        ReviewAction::ReadPage { query } => {
            let mut snapshot = read::snapshot(store).await?;
            let page = snapshot.review_page(&query).await?;
            let settings = Box::new(snapshot.settings().await?);
            snapshot.close().await?;
            Ok(ReviewResponse::Page {
                query,
                page,
                settings,
            })
        }
        ReviewAction::ReadRecord { key } => {
            let mut snapshot = read::snapshot(store).await?;
            let row = snapshot.review_record(key).await?.map(Box::new);
            let settings = Box::new(snapshot.settings().await?);
            snapshot.close().await?;
            Ok(ReviewResponse::Record { key, row, settings })
        }
    }
}
