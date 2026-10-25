//! Consistent local-working snapshots for history, review, export and entry defaults.

pub mod history;
pub mod review;

use crate::{
    metadata,
    operating::{self, OperatingState},
    store::ClientStore,
};
use clacks_model::ids::AccountId;
use clacks_sync::{
    account::{AccountMetadata, InstalledCheckpoint},
    client::ClientRecord,
    membership::MembershipSnapshot,
    record::RecordKey,
};
use sqlx::{Sqlite, Transaction};

/// An established database snapshot shared by all reads until explicitly closed or dropped.
///
/// Keep snapshots only for a bounded read/export operation. A live WAL reader
/// does not block ordinary commits, but can retain older WAL pages until released.
pub struct ReadSnapshot {
    /// Owning account for every query in this snapshot.
    account: AccountId,
    /// One checked-out connection and its ordinary SQLite read transaction.
    transaction: Transaction<'static, Sqlite>,
}

/// Establish a consistent local snapshot before returning it to the caller.
pub async fn snapshot(store: &ClientStore) -> anyhow::Result<ReadSnapshot> {
    let mut transaction = store.pool.begin().await?;
    let identity = crate::identity::load(&mut transaction)
        .await?
        .ok_or_else(|| anyhow::anyhow!("workspace identity is missing"))?;
    anyhow::ensure!(
        identity.account == store.account,
        "workspace account changed"
    );
    Ok(ReadSnapshot {
        account: store.account,
        transaction,
    })
}

impl ReadSnapshot {
    /// Read applied context and reusable sources from this existing snapshot.
    pub async fn settings(&mut self) -> anyhow::Result<crate::settings::SettingsSnapshot> {
        crate::settings::read::load_in(&mut self.transaction, self.account).await
    }

    /// Read complete current/shared/pending roles from this snapshot.
    pub async fn record(&mut self, key: RecordKey) -> anyhow::Result<Option<ClientRecord>> {
        anyhow::ensure!(
            key.account == self.account,
            "record belongs to another account"
        );
        metadata::client::load(&mut self.transaction, key).await
    }

    /// Read copied entry defaults and the complete loaded-profile baseline from this snapshot.
    pub async fn operating(&mut self) -> anyhow::Result<OperatingState> {
        operating::load_in(&mut self.transaction, self.account).await
    }

    /// Read the last completed durable receipt, independent of advertised account head.
    pub async fn checkpoint(&mut self) -> anyhow::Result<Option<InstalledCheckpoint>> {
        metadata::lineage::current_checkpoint(&mut self.transaction, self.account).await
    }

    /// Read authenticated server account and membership observations retained for reconnection.
    pub async fn account_view(
        &mut self,
    ) -> anyhow::Result<Option<(AccountMetadata, MembershipSnapshot)>> {
        metadata::account::load(&mut self.transaction, self.account).await
    }

    /// Release this reader and its retained WAL snapshot after the bounded operation completes.
    pub async fn close(self) -> anyhow::Result<()> {
        self.transaction.commit().await?;
        Ok(())
    }
}
