//! Bounded encrypted-value migration within the account transaction boundary.

use super::account::AccountRotationRequest;
use crate::secrets::{
    credentials,
    keys::{self, WrappingKeys},
    receipt::{self, EncryptedReceipt, ReceiptContext},
    rows::{CredentialRow, ReceiptEnvelopeRow},
};
use anyhow::Context;
use sqlx::{Connection, PgConnection};

/// The encrypted database families whose references must migrate together.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CiphertextFamily {
    /// Account-owned provider material.
    Credential,
    /// Issued references retained for authorization-code replay containment.
    Receipt,
}

/// Commit at most the requested number of envelopes under the owning account lock.
///
/// Returns committed rows only. A malformed envelope/context stops the batch;
/// no material is skipped and semantic revisions and authorization state are retained.
pub(crate) async fn batch(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    request: &AccountRotationRequest,
    family: CiphertextFamily,
) -> anyhow::Result<u64> {
    request.validate()?;
    let mut tx = connection.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", request.account.to_string())
        .fetch_one(&mut *tx)
        .await?;
    let target = keys::active_in(&mut tx, wrapping, request.account).await?;
    anyhow::ensure!(
        target.version == request.to_version,
        "account-key target changed during migration"
    );
    let limit = i64::from(request.batch_size.get());
    let count = match family {
        CiphertextFamily::Credential => {
            let rows = sqlx::query_file_as!(
                CredentialRow,
                "sql/secrets/rotation/credentials.sql",
                request.account.to_string(),
                request.from_version,
                limit
            )
            .fetch_all(&mut *tx)
            .await?;
            let count = u64::try_from(rows.len())?;
            for row in rows {
                credentials::reencrypt_in(&mut tx, wrapping, &row, &target)
                    .await
                    .with_context(|| {
                        format!(
                            "credential {} for account {} using key version {} could not be migrated",
                            row.credential_id, request.account, request.from_version
                        )
                    })?;
            }
            count
        }
        CiphertextFamily::Receipt => {
            let rows = sqlx::query_file_as!(
                ReceiptEnvelopeRow,
                "sql/secrets/rotation/receipts.sql",
                request.account.to_string(),
                request.from_version,
                limit
            )
            .fetch_all(&mut *tx)
            .await?;
            let count = u64::try_from(rows.len())?;
            for row in rows {
                let context = ReceiptContext {
                    account: row.account_id.parse()?,
                    code_digest: row
                        .code_digest
                        .as_slice()
                        .try_into()
                        .context("invalid stored code digest")?,
                    purpose: row
                        .purpose
                        .context("receipt is missing immutable approval context")?,
                };
                let stored = EncryptedReceipt {
                    key_version: row.key_version.context("missing receipt version")?,
                    nonce: row.nonce.context("missing receipt nonce")?,
                    ciphertext: row.ciphertext.context("missing receipt ciphertext")?,
                };
                let source =
                    keys::load_version(&mut *tx, wrapping, context.account, stored.key_version)
                        .await?;
                let references = receipt::open(&source, &context, &stored)?;
                let encrypted = receipt::seal(&target, &context, &references)?;
                sqlx::query_file!(
                    "sql/secrets/rotation/receipt_update.sql",
                    row.account_id,
                    row.code_digest,
                    encrypted.key_version,
                    encrypted.nonce,
                    encrypted.ciphertext
                )
                .execute(&mut *tx)
                .await?;
            }
            count
        }
    };
    tx.commit().await?;
    let label = match family {
        CiphertextFamily::Credential => "credential",
        CiphertextFamily::Receipt => "receipt",
    };
    metrics::counter!("clacks_secret_values_reencrypted_total", "family" => label).increment(count);
    Ok(count)
}
