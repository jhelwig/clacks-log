//! Guarded local saved-state operations for one account workspace.

use crate::{metadata::client, store::ClientStore};
use clacks_sync::{client::ClientRecord, record::RecordKey};

/// Read a complete client record from one ordinary snapshot transaction.
///
/// A missing row means no locally saved record, not proof of shared absence.
#[tracing::instrument(skip_all, name = "client_store.record.load", level = "debug", fields(record_id = %key.id))]
pub async fn load(store: &ClientStore, key: RecordKey) -> anyhow::Result<Option<ClientRecord>> {
    anyhow::ensure!(
        key.account == store.account,
        crate::error::StoreError::WrongAccount {
            expected: store.account,
            actual: key.account
        }
    );
    let mut transaction = store.pool.begin().await?;
    let record = client::load(&mut transaction, key).await?;
    transaction.commit().await?;
    Ok(record)
}
