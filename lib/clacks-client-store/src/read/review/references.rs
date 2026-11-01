//! Bounded surviving accepted references, independent of unpublished local repairs.

use super::ReviewRow;
use crate::{metadata, read::ReadSnapshot};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Accepted referrers and the full count observed in the same database snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferencePage {
    /// Current local roles for each accepted referring identity.
    pub rows: Vec<ReviewRow>,
    /// Complete accepted referrer count, independent of pagination.
    pub total: u64,
    /// Last returned identity when another page exists.
    pub next: Option<RecordId>,
}

impl ReadSnapshot {
    /// Read shared surviving references before deciding whether a deletion can retry.
    #[tracing::instrument(skip_all, name = "client_store.review.references", level = "debug", fields(record_id = %key.id, limit))]
    pub async fn review_references(
        &mut self,
        key: RecordKey,
        after: Option<RecordId>,
        limit: u32,
    ) -> anyhow::Result<ReferencePage> {
        anyhow::ensure!(
            key.account == self.account,
            "Reference target belongs to another account"
        );
        read_in(&mut self.transaction, key, after, limit).await
    }
}

/// Inspect accepted roles within a caller's existing read or guarded-write transaction.
pub(crate) async fn read_in(
    connection: &mut SqliteConnection,
    key: RecordKey,
    after: Option<RecordId>,
    limit: u32,
) -> anyhow::Result<ReferencePage> {
    anyhow::ensure!(limit > 0, "Reference page limit must be positive");
    anyhow::ensure!(
        matches!(key.id, RecordId::Tag(_) | RecordId::Equipment(_)),
        "This record kind has no supported deletion referrers"
    );
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let total = sqlx::query_file!("sql/read/review/reference_count.sql", account, kind, id)
        .fetch_one(&mut *connection)
        .await?
        .total;
    let after_kind = after.map(crate::payload::kind);
    let after_id = after.map(|id| id.to_string());
    let count = i64::from(limit) + 1;
    let selected = sqlx::query_file!(
        "sql/read/review/references.sql",
        account,
        kind,
        id,
        after_kind,
        after_id,
        count
    )
    .fetch_all(&mut *connection)
    .await?;
    let more = selected.len() > usize::try_from(limit)?;
    let mut rows = Vec::new();
    for row in selected.into_iter().take(usize::try_from(limit)?) {
        let target = RecordKey {
            account: key.account,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        let record = metadata::client::load(connection, target)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Accepted referrer has no current record"))?;
        rows.push(ReviewRow {
            record,
            review: metadata::review::load(connection, target).await?,
            authorization_hold: crate::enrollment::holds::actor_in(connection, target).await?,
        });
    }
    Ok(ReferencePage {
        next: if more {
            rows.last().map(|row| row.record.key().id)
        } else {
            None
        },
        rows,
        total: u64::try_from(total)?,
    })
}
