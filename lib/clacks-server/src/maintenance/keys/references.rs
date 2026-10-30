//! Current encrypted-reference counts, independent of key decryption.

use super::report::EncryptedCounts;
use clacks_model::ids::AccountId;
use sqlx::PgConnection;

/// Count both encrypted schema families within the caller's snapshot or account lock.
pub(crate) async fn count(
    connection: &mut PgConnection,
    account: AccountId,
    version: i64,
) -> anyhow::Result<EncryptedCounts> {
    let row = sqlx::query_file!(
        "sql/secrets/rotation/count.sql",
        account.to_string(),
        version
    )
    .fetch_one(connection)
    .await?;
    Ok(EncryptedCounts {
        credentials: row.credentials.try_into()?,
        receipts: row.receipts.try_into()?,
    })
}
