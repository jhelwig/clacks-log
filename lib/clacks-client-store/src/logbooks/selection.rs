//! Durable client-local view selection, independent of shared tag records.

use crate::{local_state, metadata::client, store::ClientStore};
use anyhow::Context;
use clacks_model::ids::TagId;
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

/// Key owned by the contact-view selector in the shared local-state store.
const SELECTED_VIEW_KEY: &str = "logbooks.selected_view";

/// Contact view and default membership used by one local workspace.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum SelectedView {
    /// Every active contact, with no default tag for new contacts.
    #[default]
    AllContacts,
    /// Active contacts with no tags, also supplying no default tag.
    Untagged,
    /// The named tag's contacts and default membership for new contacts.
    Logbook(TagId),
}

/// Commit a view choice, rejecting named targets without a live working tag.
///
/// Use for explicit selector changes. A failed validation/write preserves the
/// previous selection; successful no-ops return it without a mutation event.
#[tracing::instrument(skip_all, name = "client_store.logbook.select", fields(account_id = %store.account, view = ?next))]
pub async fn select(store: &ClientStore, next: SelectedView) -> anyhow::Result<SelectedView> {
    let mut transaction = store.write().await?;
    let current = load_in(&mut transaction).await?;
    if let SelectedView::Logbook(id) = next {
        let key = RecordKey {
            account: store.account,
            id: RecordId::Tag(id),
        };
        let target = client::load(&mut transaction, key).await?;
        anyhow::ensure!(
            target.as_ref().is_some_and(|row| matches!(
                row.working_state(),
                Some(RecordState::Live(RecordValue::Tag(_)))
            )),
            "logbook {id} is not available as a live logbook"
        );
    }
    if next != current {
        local_state::set_in(
            &mut transaction,
            SELECTED_VIEW_KEY,
            &serde_json::to_string(&next)?,
        )
        .await?;
    }
    transaction.commit().await?;
    if next != current {
        tracing::info!(event = "logbook.view.selected", account_id = %store.account, view = ?next);
    }
    Ok(next)
}

/// Replace a known-deleted selection with All contacts and return its tag ID.
///
/// Call at workspace startup before publishing initial views. Missing/unknown
/// tag knowledge preserves the choice; invalid stored state remains an error.
#[tracing::instrument(skip_all, name = "client_store.logbook.restore", level = "debug", fields(account_id = %store.account))]
pub async fn restore(store: &ClientStore) -> anyhow::Result<Option<TagId>> {
    let mut transaction = store.write().await?;
    let selected = load_in(&mut transaction).await?;
    let mut fallback = None;
    if let SelectedView::Logbook(id) = selected {
        let key = RecordKey {
            account: store.account,
            id: RecordId::Tag(id),
        };
        let record = client::load(&mut transaction, key).await?;
        if record
            .as_ref()
            .is_some_and(|value| matches!(value.working_state(), Some(RecordState::Deleted)))
        {
            fallback_deleted_in(&mut transaction, key).await?;
            fallback = Some(id);
        }
    }
    transaction.commit().await?;
    Ok(fallback)
}

/// Read the native preference, using All contacts only when its key is absent.
///
/// Invalid stored values are errors rather than permission to reset the choice.
pub(crate) async fn load_in(connection: &mut SqliteConnection) -> anyhow::Result<SelectedView> {
    local_state::get_in(connection, SELECTED_VIEW_KEY)
        .await?
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .context("decoding the selected contact view")
        .map(Option::unwrap_or_default)
}

/// Switch to All contacts if the deleted tag is the current selection.
///
/// The caller establishes working deletion. This performs no commit or success
/// reporting, and unrelated record kinds cannot invalidate a tag selection.
pub(crate) async fn fallback_deleted_in(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<()> {
    let RecordId::Tag(id) = key.id else {
        return Ok(());
    };
    if load_in(connection).await? == SelectedView::Logbook(id) {
        let value = serde_json::to_string(&SelectedView::AllContacts)?;
        local_state::set_in(connection, SELECTED_VIEW_KEY, &value).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
