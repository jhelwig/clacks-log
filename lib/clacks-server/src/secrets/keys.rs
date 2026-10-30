//! Deployment wrapping keys and database-backed account encryption keys.

#[cfg(test)]
mod tests;

use super::envelope::{self, WrappedKeyEnvelope};
use crate::config::WrappingKeyConfig;
use anyhow::Context;
use clacks_model::ids::AccountId;
use rand::{RngCore, rngs::OsRng};
use sqlx::PgConnection;
use std::collections::BTreeMap;
use zeroize::Zeroizing;

/// Protected deployment keys indexed by the IDs retained in account envelopes.
pub(crate) struct WrappingKeys {
    /// Version selected for newly wrapped account keys.
    pub(crate) active: String,
    /// Retained fixed-size deployment material.
    keys: BTreeMap<String, Zeroizing<[u8; 32]>>,
}

impl WrappingKeys {
    /// Load exact-length key files once without regenerating missing material.
    pub fn load(config: &WrappingKeyConfig) -> anyhow::Result<Self> {
        let mut keys = BTreeMap::new();
        for (id, file) in &config.files {
            let bytes =
                Zeroizing::new(std::fs::read(file).context("cannot read deployment wrapping key")?);
            let key = bytes
                .as_slice()
                .try_into()
                .context("deployment wrapping key must contain exactly 32 raw bytes")?;
            keys.insert(id.clone(), Zeroizing::new(key));
        }
        anyhow::ensure!(
            keys.contains_key(&config.active),
            "active deployment wrapping key is not configured"
        );
        Ok(Self {
            active: config.active.clone(),
            keys,
        })
    }

    /// Resolve one exact retained deployment-key reference without fallback.
    fn key(&self, id: &str) -> anyhow::Result<&[u8; 32]> {
        self.keys
            .get(id)
            .map(|key| &**key)
            .with_context(|| format!("required deployment wrapping key {id:?} is unavailable"))
    }
}

/// Decrypted account key retained only for the owning secret operation.
pub(crate) struct AccountKey {
    /// Account authenticated by the key envelope.
    pub(super) account: AccountId,
    /// Positive version referenced by encrypted account records.
    pub(super) version: i64,
    /// Fixed-size account key, erased when released.
    pub(super) bytes: Zeroizing<[u8; 32]>,
}

impl AccountKey {
    /// Return the account-key version used by this operation.
    pub(crate) fn version(&self) -> i64 {
        self.version
    }
}

/// Stored account-key identity and complete encrypted value.
struct KeyRow {
    /// Account-owned key version.
    key_version: i64,
    /// Whole versioned wrapping envelope.
    wrapped_envelope: String,
}

/// Authenticated identity and codec metadata of an account-key envelope.
#[derive(serde::Serialize)]
struct KeyContext {
    /// Distinguishes this envelope from other encrypted object types.
    format: &'static str,
    /// Independently expected owning account.
    account: AccountId,
    /// Independently expected account-key version.
    version: i64,
    /// Canonical codec version, kind and deployment-key reference.
    storage_header: String,
}

/// Bind codec metadata to independently expected account/key identity.
fn associated(account: AccountId, version: i64, wrapping_key: &str) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(&KeyContext {
        format: "clacks.account-key.v1",
        account,
        version,
        storage_header: envelope::account_key_header(wrapping_key),
    })?)
}

/// Load or provision the active key within an already account-locked transaction.
///
/// Call before encrypting a value that commits in the same transaction. Retained
/// historical keys without an active key are an error, not a new account.
#[tracing::instrument(skip_all, name = "account.secrets.key", level = "debug", fields(account_id = %account))]
pub(crate) async fn active_in(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    account: AccountId,
) -> anyhow::Result<AccountKey> {
    let row = sqlx::query_file_as!(KeyRow, "sql/secrets/current.sql", account.to_string())
        .fetch_optional(&mut *connection)
        .await?;
    if let Some(row) = row {
        return unwrap_key(wrapping, account, row);
    }
    let exists = sqlx::query_file!("sql/secrets/exists.sql", account.to_string())
        .fetch_one(&mut *connection)
        .await?;
    anyhow::ensure!(
        !exists.present,
        "account has retained keys but no active key"
    );
    create_in(connection, wrapping, account, 1).await
}

/// Generate an unused positive account-key version under the active deployment key.
///
/// The caller owns the account lock and commits any previous-key deactivation
/// with this insertion.
pub(crate) async fn create_in(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    account: AccountId,
    version: i64,
) -> anyhow::Result<AccountKey> {
    anyhow::ensure!(version > 0, "account key version must be positive");
    let mut bytes = Zeroizing::new([0u8; 32]);
    OsRng.try_fill_bytes(bytes.as_mut_slice())?;
    let encrypted = wrap(wrapping, account, version, bytes.as_slice())?.encode();
    sqlx::query_file!(
        "sql/secrets/insert.sql",
        account.to_string(),
        version,
        encrypted
    )
    .execute(connection)
    .await?;
    Ok(AccountKey {
        account,
        version,
        bytes,
    })
}

/// Wrap unchanged account-key bytes using the configured active deployment key.
pub(crate) fn wrap(
    wrapping: &WrappingKeys,
    account: AccountId,
    version: i64,
    bytes: &[u8],
) -> anyhow::Result<WrappedKeyEnvelope> {
    let context = associated(account, version, &wrapping.active)?;
    let sealed = super::encrypt(wrapping.key(&wrapping.active)?, &context, bytes)?;
    WrappedKeyEnvelope::new(wrapping.active.clone(), sealed)
}

/// Load the exact account-key version referenced by an encrypted record.
pub(crate) async fn load_version<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    wrapping: &WrappingKeys,
    account: AccountId,
    version: i64,
) -> anyhow::Result<AccountKey> {
    let row = sqlx::query_file_as!(
        KeyRow,
        "sql/secrets/version.sql",
        account.to_string(),
        version
    )
    .fetch_one(executor)
    .await?;
    unwrap_key(wrapping, account, row)
}

/// Authenticate the expected account/version and stored codec metadata before returning key bytes.
fn unwrap_key(
    wrapping: &WrappingKeys,
    account: AccountId,
    row: KeyRow,
) -> anyhow::Result<AccountKey> {
    anyhow::ensure!(row.key_version > 0, "account key version must be positive");
    let value = WrappedKeyEnvelope::parse(&row.wrapped_envelope)?;
    let context = associated(account, row.key_version, value.wrapping_key())?;
    let bytes = Zeroizing::new(super::decrypt(
        wrapping.key(value.wrapping_key())?,
        &context,
        value.sealed().nonce,
        &value.sealed().bytes,
    )?);
    let key = bytes
        .as_slice()
        .try_into()
        .context("invalid decrypted account key length")?;
    Ok(AccountKey {
        account,
        version: row.key_version,
        bytes: Zeroizing::new(key),
    })
}

/// Rewrap one exact account-key version while preserving its bytes and active designation.
///
/// Requires the caller's account lock and maintenance authorization.
pub(crate) async fn rewrap_in(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    account: AccountId,
    version: i64,
) -> anyhow::Result<()> {
    let key = load_version(&mut *connection, wrapping, account, version).await?;
    let encrypted = wrap(wrapping, account, version, key.bytes.as_slice())?.encode();
    sqlx::query_file!(
        "sql/secrets/rotation/wrapping_update.sql",
        account.to_string(),
        version,
        encrypted
    )
    .execute(connection)
    .await?;
    Ok(())
}
