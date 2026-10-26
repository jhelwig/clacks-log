//! Read the selected view and logbook tag records from one database snapshot.

use super::{LogbookSnapshot, selection};
use crate::metadata::client;
use clacks_model::ids::AccountId;
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Read the current catalog without treating an unavailable tag as a deleted one.
pub(crate) async fn load_in(
    connection: &mut SqliteConnection,
    account: AccountId,
) -> anyhow::Result<LogbookSnapshot> {
    let selected_view = selection::load_in(connection).await?;
    let account_text = account.to_string();
    let mut tags = Vec::new();
    for row in sqlx::query_file!("sql/logbooks/catalog.sql", account_text)
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account,
            id: RecordId::Tag(row.record_id.parse()?),
        };
        tags.push(client::load(connection, key).await?.ok_or_else(|| {
            anyhow::anyhow!("tag record disappeared within its catalog snapshot")
        })?);
    }
    Ok(LogbookSnapshot {
        selected_view,
        tags,
    })
}
