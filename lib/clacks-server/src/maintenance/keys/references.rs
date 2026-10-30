//! Current encrypted-reference counts, independent of key decryption.

use clacks_model::ids::AccountId;
use sqlx::PgConnection;

/// Count receipt references within the caller's snapshot or account lock.
pub(crate) async fn count(
    connection: &mut PgConnection,
    account: AccountId,
    version: i64,
) -> anyhow::Result<u64> {
    let row = sqlx::query_file!(
        "sql/secrets/rotation/count.sql",
        account.to_string(),
        version
    )
    .fetch_one(connection)
    .await?;
    Ok(row.receipts.try_into()?)
}

/// Nonsecret account-key metadata and current references for one version.
#[derive(Debug, serde::Serialize)]
pub(crate) struct VersionStatus {
    /// Retained account-key version.
    pub version: i64,
    /// Whether new writes select this version.
    pub active: bool,
    /// Deployment key required to decrypt this account key.
    pub wrapping_key_id: String,
    /// Current values still requiring this version.
    pub receipts: u64,
}

/// Aggregated current account-key usage of a deployment key.
#[derive(Debug, serde::Serialize)]
pub(crate) struct WrappingStatus {
    /// Public deployment-key ID.
    pub wrapping_key_id: String,
    /// Active and inactive wrapped account keys.
    pub account_keys: u64,
}

/// Current database metadata; it makes no claim about external processes/backups.
#[derive(Debug, serde::Serialize)]
pub(crate) enum KeyStatus {
    /// Aggregates across the deployment without enumerating individual accounts.
    Deployment {
        /// Counts grouped by deployment wrapping key.
        wrapping_keys: Vec<WrappingStatus>,
    },
    /// Versions for an existing account; an empty list explicitly means keyless.
    Account {
        /// Account selected by the operator.
        account: AccountId,
        /// All retained versions and their current encrypted references.
        versions: Vec<VersionStatus>,
    },
}

/// Read current key/reference metadata in one read-only snapshot without decryption.
///
/// Missing accounts are errors. An existing keyless account is reported without
/// provisioning material. No wrapping-key files are loaded by this operation.
pub(crate) async fn status(
    connection: &mut PgConnection,
    account: Option<AccountId>,
) -> anyhow::Result<KeyStatus> {
    use sqlx::Connection;
    let mut tx = connection.begin().await?;
    sqlx::query_file!("sql/secrets/rotation/read_snapshot.sql")
        .execute(&mut *tx)
        .await?;
    let status = if let Some(account) = account {
        let exists = sqlx::query_file!(
            "sql/secrets/rotation/account_exists.sql",
            account.to_string()
        )
        .fetch_optional(&mut *tx)
        .await?;
        anyhow::ensure!(exists.is_some(), "account does not exist");
        let rows = sqlx::query_file!(
            "sql/secrets/rotation/account_status.sql",
            account.to_string()
        )
        .fetch_all(&mut *tx)
        .await?;
        let versions = rows
            .into_iter()
            .map(|row| {
                Ok(VersionStatus {
                    version: row.version,
                    active: row.active,
                    wrapping_key_id: crate::secrets::envelope::wrapping_id(
                        &row.wrapping_key_reference,
                    )?,
                    receipts: row.receipts.try_into()?,
                })
            })
            .collect::<anyhow::Result<_>>()?;
        KeyStatus::Account { account, versions }
    } else {
        let rows = sqlx::query_file!("sql/secrets/rotation/deployment_status.sql")
            .fetch_all(&mut *tx)
            .await?;
        let wrapping_keys = rows
            .into_iter()
            .map(|row| {
                Ok(WrappingStatus {
                    wrapping_key_id: crate::secrets::envelope::wrapping_id(
                        &row.wrapping_key_reference,
                    )?,
                    account_keys: row.account_keys.try_into()?,
                })
            })
            .collect::<anyhow::Result<_>>()?;
        KeyStatus::Deployment { wrapping_keys }
    };
    tx.commit().await?;
    Ok(status)
}
