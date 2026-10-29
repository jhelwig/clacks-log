//! Deployment wrapping keys and database-backed account encryption keys.

use crate::config::WrappingKeyConfig;
use anyhow::Context;
use clacks_model::ids::AccountId;
use rand::{RngCore, rngs::OsRng};
use sqlx::PgPool;
use std::collections::BTreeMap;
use zeroize::Zeroizing;

/// Protected deployment keys indexed by the IDs retained in account envelopes.
pub(crate) struct WrappingKeys {
    /// Version selected for newly wrapped account keys.
    active: String,
    /// Retained raw deployment material, indexed by configured version.
    keys: BTreeMap<String, Zeroizing<Vec<u8>>>,
}

impl WrappingKeys {
    /// Load exact-length key files once without regenerating missing material.
    pub fn load(config: &WrappingKeyConfig) -> anyhow::Result<Self> {
        let mut keys = BTreeMap::new();
        for (id, file) in &config.files {
            let bytes =
                Zeroizing::new(std::fs::read(file).context("cannot read deployment wrapping key")?);
            anyhow::ensure!(
                bytes.len() == 32,
                "deployment wrapping key must contain exactly 32 raw bytes"
            );
            keys.insert(id.clone(), bytes);
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

    /// Resolve a retained envelope's key version without falling back to another key.
    fn key(&self, id: &str) -> anyhow::Result<&[u8]> {
        self.keys
            .get(id)
            .map(|key| key.as_slice())
            .context("required deployment wrapping key is unavailable")
    }
}

/// Decrypted account key retained only for the owning secret operation.
pub(crate) struct AccountKey {
    /// Account authenticated by the key envelope.
    pub account: AccountId,
    /// Version referenced by encrypted account records.
    pub version: i64,
    /// Raw account key, erased when released.
    pub(super) bytes: Zeroizing<Vec<u8>>,
}

/// Stored account-key fields selected together from the production schema.
struct KeyRow {
    /// Account-owned key version.
    key_version: i64,
    /// Deployment key required for this envelope.
    wrapping_key_id: String,
    /// Nonce retained alongside the authenticated ciphertext.
    wrapping_nonce: Vec<u8>,
    /// Encrypted raw account key and authentication tag.
    wrapped_key: Vec<u8>,
}

/// Authenticated identity of an account-key envelope.
#[derive(serde::Serialize)]
struct KeyContext {
    /// Distinguishes this envelope from other encrypted object types.
    format: &'static str,
    /// Owning account.
    account: AccountId,
    /// Exact account-key version.
    version: i64,
}

/// Load or provision the account key while holding the normal account-row lock.
#[tracing::instrument(skip_all, name = "account.secrets.key", level = "debug", fields(account_id = %account))]
pub(crate) async fn account_key(
    pool: &PgPool,
    wrapping: &WrappingKeys,
    account: AccountId,
) -> anyhow::Result<AccountKey> {
    let mut transaction = pool.begin().await?;
    let account_text = account.to_string();
    sqlx::query_file!("sql/account_lock.sql", account_text)
        .fetch_one(&mut *transaction)
        .await?;
    let row = sqlx::query_file_as!(KeyRow, "sql/secrets/current.sql", account_text)
        .fetch_optional(&mut *transaction)
        .await?;
    let key = match row {
        Some(row) => unwrap_key(wrapping, account, row)?,
        None => {
            let mut bytes = Zeroizing::new(vec![0u8; 32]);
            OsRng.try_fill_bytes(bytes.as_mut_slice())?;
            let version = 1i64;
            let associated = serde_json::to_vec(&KeyContext {
                format: "clacks.account-key.v1",
                account,
                version,
            })?;
            let encrypted = super::encrypt(
                wrapping.key(&wrapping.active)?,
                &associated,
                bytes.as_slice(),
            )?;
            sqlx::query_file!(
                "sql/secrets/insert.sql",
                account_text,
                version,
                wrapping.active,
                encrypted.nonce.as_slice(),
                encrypted.bytes
            )
            .execute(&mut *transaction)
            .await?;
            AccountKey {
                account,
                version,
                bytes,
            }
        }
    };
    transaction.commit().await?;
    Ok(key)
}

/// Load the exact key version referenced by a retained encrypted record.
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

/// Authenticate the account/version binding before returning a decrypted key.
fn unwrap_key(
    wrapping: &WrappingKeys,
    account: AccountId,
    row: KeyRow,
) -> anyhow::Result<AccountKey> {
    let nonce: [u8; 12] = row
        .wrapping_nonce
        .as_slice()
        .try_into()
        .context("invalid stored wrapping nonce")?;
    let associated = serde_json::to_vec(&KeyContext {
        format: "clacks.account-key.v1",
        account,
        version: row.key_version,
    })?;
    let bytes = Zeroizing::new(super::decrypt(
        wrapping.key(&row.wrapping_key_id)?,
        &associated,
        nonce,
        &row.wrapped_key,
    )?);
    anyhow::ensure!(bytes.len() == 32, "invalid decrypted account key length");
    Ok(AccountKey {
        account,
        version: row.key_version,
        bytes,
    })
}
