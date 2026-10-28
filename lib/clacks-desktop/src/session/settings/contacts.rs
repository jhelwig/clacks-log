//! Execute actual contact reads and guarded edits on the workspace runtime.

#[cfg(test)]
mod tests;

use clacks_client_store::{
    read::{
        self,
        contacts::{ContactPage, ContactPageQuery},
        query_state::{self, QueryState},
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
    },
    /// Delete the reviewed account-owned contact under its expected basis.
    Delete { expected: Box<ClientRecord> },
}

/// An authoritative contact result, independent of any following settings refresh.
#[derive(Debug)]
pub(crate) enum ContactResponse {
    /// Stored expression text, independent of parsing and current rows.
    QueryState(QueryState),
    /// The exact preference pair durably acknowledged by the store.
    QueryStateSaved(QueryState),
    /// Real local working rows.
    Page(ContactPage),
    /// Current stored record, absent when unavailable locally.
    Loaded(Option<ClientRecord>),
    /// Committed, unchanged or stale contact facts.
    Saved(SaveOutcome),
    /// Committed or stale contact deletion.
    Deleted(SaveOutcome),
}

/// Execute a contact operation without moving database work into redraw callbacks.
#[tracing::instrument(skip_all, name = "desktop.contacts")]
pub(crate) async fn execute(
    store: &ClientStore,
    action: ContactAction,
) -> anyhow::Result<ContactResponse> {
    match action {
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
        ContactAction::Load { id } => Ok(ContactResponse::Loaded(
            save::load(store, key(store, id)).await?,
        )),
        ContactAction::Save { expected, next } => {
            let creating = expected.is_none();
            let record_key = key(store, next.id);
            let next = RecordState::Live(RecordValue::Qso(next));
            let mut outcome =
                save::save(store, record_key, expected.as_deref(), next.clone()).await?;
            if creating
                && let SaveOutcome::Stale {
                    current: Some(current),
                } = &outcome
                && current
                    .working_state()
                    .is_some_and(|value| value.same_values(&next))
            {
                tracing::info!(event = "contact.create_retry_acknowledged", account_id = %store.account_id(), record_id = %record_key.id);
                outcome = SaveOutcome::Unchanged(current.clone());
            }
            if matches!(outcome, SaveOutcome::Stale { .. }) {
                tracing::info!(event = "contact.save.stale", account_id = %store.account_id(), record_id = %record_key.id);
            }
            Ok(ContactResponse::Saved(outcome))
        }
        ContactAction::Delete { expected } => {
            anyhow::ensure!(
                matches!(expected.key.id, RecordId::Qso(_)),
                "Only a contact can be deleted here"
            );
            let outcome =
                save::save(store, expected.key, Some(&expected), RecordState::Deleted).await?;
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
