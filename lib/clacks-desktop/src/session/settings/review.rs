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
    /// Retry a previously blocked deletion after shared reference repair.
    RetryDeletion {
        /// Exact saved target compared by the operator.
        expected: Box<clacks_sync::client::ClientRecord>,
        /// Exact persisted deletion rejection.
        shown: Box<clacks_sync::client::ReviewInput>,
    },
    /// Read surviving accepted referrers, independent of local working repairs.
    ReadReferences {
        /// Target whose deletion is blocked.
        key: RecordKey,
        /// Exclusive previous referring identity.
        after: Option<clacks_sync::record::RecordId>,
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

impl ReviewAction {
    /// Local operation name without containing-action context or request payloads.
    pub(crate) fn diagnostic_name(&self) -> &'static str {
        match self {
            Self::Resolve { .. } => "resolve",
            Self::RetryDeletion { .. } => "retry_deletion",
            Self::ReadReferences { .. } => "read_references",
            Self::ReadPage { .. } => "read_page",
            Self::ReadRecord { .. } => "read_record",
        }
    }
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
    /// Guarded retry result for the original deletion target.
    Retried {
        /// Account-qualified target.
        key: RecordKey,
        /// Actual guarded transition or remaining shared references.
        outcome: Box<clacks_client_store::synchronization::RetryDeletionOutcome>,
    },
    /// Bounded accepted referrers for a specific target.
    References {
        /// Account-qualified deletion target.
        key: RecordKey,
        /// Continuation used by this request.
        after: Option<clacks_sync::record::RecordId>,
        /// Actual accepted references and full count.
        page: clacks_client_store::read::review::references::ReferencePage,
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
        ReviewAction::RetryDeletion { expected, shown } => {
            let key = expected.key();
            let outcome =
                clacks_client_store::synchronization::retry_deletion(store, &expected, &shown)
                    .await?;
            Ok(ReviewResponse::Retried {
                key,
                outcome: Box::new(outcome),
            })
        }
        ReviewAction::ReadReferences { key, after } => {
            let mut snapshot = read::snapshot(store).await?;
            let page = snapshot.review_references(key, after, 100).await?;
            snapshot.close().await?;
            Ok(ReviewResponse::References { key, after, page })
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
