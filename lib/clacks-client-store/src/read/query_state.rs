//! Workspace-local history text, including unfinished edits and the last applied expression.

use super::ReadSnapshot;
use crate::{local_state, store::ClientStore};
use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Local preference key shared by every selected contact view.
const KEY: &str = "history.query";

/// Operator-entered text retained independently of parsing and result rows.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct QueryState {
    /// Current edit, which may be incomplete or invalid.
    pub edited: String,
    /// Last expression whose result page was accepted for display.
    pub applied: String,
}

/// Versioned persistence envelope; future formats must not silently reset a query.
#[derive(Deserialize, Serialize)]
struct StoredState {
    /// Supported local preference representation.
    version: u32,
    /// The two independently meaningful expressions.
    state: QueryState,
}

impl ReadSnapshot {
    /// Restore history text, defaulting only when no preference has been saved.
    ///
    /// An unsupported version or malformed stored value is an actionable error.
    pub async fn history_query_state(&mut self) -> anyhow::Result<QueryState> {
        let Some(value) = local_state::get_in(&mut self.transaction, KEY).await? else {
            return Ok(QueryState::default());
        };
        let stored: StoredState =
            serde_json::from_str(&value).context("decoding the local history query")?;
        anyhow::ensure!(
            stored.version == 1,
            "Unsupported history query state version {}",
            stored.version
        );
        Ok(stored.state)
    }
}

/// Save both expressions atomically without creating synchronized changes.
///
/// Parsing is intentionally separate: an unfinished edit must survive restart.
#[tracing::instrument(skip_all, name = "client_store.history_query.save", level = "debug", fields(account_id = %store.account))]
pub async fn save(store: &ClientStore, next: &QueryState) -> anyhow::Result<()> {
    let value = serde_json::to_string(&StoredState {
        version: 1,
        state: next.clone(),
    })?;
    let mut transaction = store.write().await?;
    local_state::set_in(&mut transaction, KEY, &value).await?;
    transaction.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A future format must not be mistaken for an absent preference.
    #[tokio::test]
    async fn unsupported_query_state_is_not_reset() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let store = ClientStore::create(
            &directory.path().join("state.sqlite"),
            Default::default(),
            Default::default(),
        )
        .await?;
        let mut transaction = store.write().await?;
        local_state::set_in(
            &mut transaction,
            KEY,
            r#"{"version":2,"state":{"edited":"unfinished","applied":"mode:CW"}}"#,
        )
        .await?;
        transaction.commit().await?;
        let mut snapshot = crate::read::snapshot(&store).await?;
        assert!(
            snapshot
                .history_query_state()
                .await
                .unwrap_err()
                .to_string()
                .contains("Unsupported history query state version 2")
        );
        snapshot.close().await?;
        store.close().await?;
        Ok(())
    }
}
