//! Execute actual contact reads and guarded edits on the workspace runtime.

pub(crate) mod membership;

#[cfg(test)]
mod tests;

use clacks_client_store::{
    read::{
        self,
        contacts::{ContactPage, ContactPageQuery},
        query_state::{self, QueryState},
        worked_before::WorkedBeforeQuery,
    },
    save::{self, SaveOutcome},
    store::ClientStore,
};
use clacks_model::{contact::Qso, ids::QsoId};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};

/// Owned contact work admitted through the existing session channel.
pub(crate) enum ContactAction {
    /// Read prior contacts for the current literal contacted-callsign prefix.
    WorkedBefore { query: WorkedBeforeQuery },
    /// Restore unfinished and applied history expressions.
    LoadQueryState,
    /// Save local history text without changing shared settings.
    SaveQueryState { state: QueryState },
    /// Read one bounded selected-view page.
    Page { query: ContactPageQuery },
    /// Load an actual record and expected editing basis.
    Load { id: QsoId },
    /// Save validated facts under the displayed expected record.
    Save {
        expected: Option<Box<ClientRecord>>,
        next: Box<Qso>,
        directory: Option<crate::directory::lookup::LookupDemand>,
    },
    /// Read selected contacts and catalog from one snapshot.
    PrepareMembership {
        ids: std::collections::BTreeSet<QsoId>,
    },
    /// Apply the captured membership intent.
    ChangeMembership(membership::MembershipSubmission),
    /// Delete the reviewed account-owned contact under its expected basis.
    Delete { expected: Box<ClientRecord> },
}

impl ContactAction {
    /// Local operation name without containing-action context or request payloads.
    pub(crate) fn diagnostic_name(&self) -> &'static str {
        match self {
            Self::WorkedBefore { .. } => "worked_before",
            Self::LoadQueryState => "query_state.load",
            Self::SaveQueryState { .. } => "query_state.save",
            Self::Page { .. } => "page",
            Self::Load { .. } => "load",
            Self::Save { .. } => "save",
            Self::PrepareMembership { .. } => "membership.prepare",
            Self::ChangeMembership(_) => "membership.change",
            Self::Delete { .. } => "delete",
        }
    }

    /// Classify cancellable contact queries for their shared duration metric.
    pub(super) fn query_kind(&self) -> Option<&'static str> {
        match self {
            Self::Page { .. } => Some("history"),
            Self::WorkedBefore { .. } => Some("worked_before"),
            _ => None,
        }
    }
}

/// An authoritative contact result, independent of any following settings refresh.
#[derive(Debug)]
pub(crate) enum ContactResponse {
    /// Working contacts supplying the shared Worked Before view.
    WorkedBefore(ContactPage),
    /// Stored expression text, independent of parsing and current rows.
    QueryState(QueryState),
    /// The exact preference pair durably acknowledged by the store.
    QueryStateSaved(QueryState),
    /// Real local working rows.
    Page(ContactPage),
    /// Current stored record, absent when unavailable locally.
    Loaded(
        Option<ClientRecord>,
        Option<Box<clacks_model::directory::values::DirectorySnapshot>>,
    ),
    /// Committed, unchanged or stale contact facts.
    Saved(SaveOutcome),
    /// Captured contact bases and catalog.
    MembershipPrepared(membership::MembershipBasis),
    /// Record-specific membership results.
    MembershipChanged(clacks_client_store::save::tags::TagChangeReport),
    /// Committed or stale contact deletion.
    Deleted(SaveOutcome),
}

/// Execute a contact operation without moving database work into redraw callbacks.
#[tracing::instrument(skip_all, name = "desktop.contacts")]
pub(crate) async fn execute(
    store: &ClientStore,
    action: ContactAction,
    lookup: Option<crate::directory::lookup::LookupClient>,
    changes: tokio::sync::watch::Sender<()>,
) -> anyhow::Result<ContactResponse> {
    match action {
        ContactAction::WorkedBefore { query } => {
            let mut snapshot = read::snapshot(store).await?;
            let page = snapshot.worked_before(&query).await?;
            snapshot.close().await?;
            Ok(ContactResponse::WorkedBefore(page))
        }
        ContactAction::LoadQueryState => {
            let mut snapshot = read::snapshot(store).await?;
            let state = snapshot.history_query_state().await?;
            snapshot.close().await?;
            Ok(ContactResponse::QueryState(state))
        }
        ContactAction::SaveQueryState { state } => {
            query_state::save(store, &state).await?;
            Ok(ContactResponse::QueryStateSaved(state))
        }
        ContactAction::Page { query } => {
            let mut snapshot = read::snapshot(store).await?;
            let page = snapshot.contact_page(&query).await?;
            snapshot.close().await?;
            Ok(ContactResponse::Page(page))
        }
        ContactAction::Load { id } => {
            let mut read = read::snapshot(store).await?;
            let record = read.record(key(store, id)).await?;
            let directory = read
                .record(RecordKey {
                    account: store.account_id(),
                    id: RecordId::Directory(id),
                })
                .await?;
            let snapshot = match (
                record.as_ref().and_then(|row| row.working_state()),
                directory.as_ref().and_then(|row| row.working_state()),
            ) {
                (
                    Some(RecordState::Live(RecordValue::Qso(qso))),
                    Some(RecordState::Live(RecordValue::Directory(directory))),
                ) => directory
                    .snapshot(&clacks_model::directory::values::LookupKey::from_recorded(
                        &qso.contacted_station().callsign,
                    ))
                    .cloned(),
                _ => None,
            };
            read.close().await?;
            Ok(ContactResponse::Loaded(record, snapshot.map(Box::new)))
        }
        ContactAction::Save {
            expected,
            next,
            directory,
        } => {
            let record_key = key(store, next.id());
            let prepared = lookup.zip(directory).map(|(lookup, demand)| {
                crate::directory::lookup::persistence::prepare(
                    lookup,
                    store.clone(),
                    next.id(),
                    demand,
                    changes,
                )
            });
            let snapshot = prepared
                .as_ref()
                .and_then(|prepared| prepared.ready_snapshot());
            let outcome = save::contacts::save(
                store,
                expected.as_deref(),
                *next,
                snapshot.map(|value| *value),
            )
            .await?;
            if let Some(prepared) = prepared {
                prepared.settle(matches!(
                    outcome,
                    SaveOutcome::Saved(_) | SaveOutcome::Unchanged(_)
                ));
            }
            if matches!(outcome, SaveOutcome::Stale { .. }) {
                tracing::info!(event = "contact.save.stale", account_id = %store.account_id(), record_id = %record_key.id);
            }
            Ok(ContactResponse::Saved(outcome))
        }
        ContactAction::PrepareMembership { ids } => Ok(ContactResponse::MembershipPrepared(
            membership::prepare(store, &ids).await?,
        )),
        ContactAction::ChangeMembership(submission) => Ok(ContactResponse::MembershipChanged(
            membership::apply(store, submission).await?,
        )),
        ContactAction::Delete { expected } => {
            anyhow::ensure!(
                matches!(expected.key().id, RecordId::Qso(_)),
                "Only a contact can be deleted here"
            );
            let outcome =
                save::save(store, expected.key(), Some(&expected), RecordState::Deleted).await?;
            Ok(ContactResponse::Deleted(outcome))
        }
    }
}

/// Construct the account-qualified native contact key for this workspace.
fn key(store: &ClientStore, id: QsoId) -> RecordKey {
    RecordKey {
        account: store.account_id(),
        id: RecordId::Qso(id),
    }
}
