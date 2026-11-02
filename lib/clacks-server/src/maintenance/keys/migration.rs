//! Bounded encrypted-value migration within the account transaction boundary.

use super::account::AccountRotationRequest;
use crate::secrets::{
    envelope::ReceiptEnvelope,
    keys::{self, WrappingKeys},
    receipt::{self, ReceiptContext},
    rows::ReceiptEnvelopeRow,
};
use anyhow::Context;
use sqlx::{Connection, PgConnection};

/// Commit at most the requested number of envelopes under the owning account lock.
///
/// Returns committed rows only. A malformed envelope/context stops the batch;
/// no material is skipped and semantic revisions and authorization state are retained.
pub(crate) async fn batch(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    request: &AccountRotationRequest,
) -> anyhow::Result<u64> {
    let mut tx = connection.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", request.account().to_string())
        .fetch_one(&mut *tx)
        .await?;
    let target = keys::active_in(&mut tx, wrapping, request.account()).await?;
    anyhow::ensure!(
        target.version() == request.target_version(),
        "account-key target changed during migration"
    );
    let limit = i64::from(request.batch_size().get());
    let rows = sqlx::query_file_as!(
        ReceiptEnvelopeRow,
        "sql/secrets/rotation/receipts.sql",
        request.account().to_string(),
        request.source_version(),
        limit
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut count = u64::try_from(rows.len())?;
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
        let stored = ReceiptEnvelope::parse(row.receipt.as_deref().context("missing receipt")?)?;
        let source =
            keys::load_version(&mut *tx, wrapping, context.account, stored.key_version()).await?;
        let references = receipt::open(&source, &context, &stored)?;
        let encrypted = receipt::seal(&target, &context, &references)?.encode();
        sqlx::query_file!(
            "sql/secrets/rotation/receipt_update.sql",
            row.account_id,
            row.code_digest,
            encrypted
        )
        .execute(&mut *tx)
        .await?;
    }

    let remaining = limit - i64::try_from(count)?;
    let rows = sqlx::query_file_as!(
        crate::store::rows::directory::MaterialRow,
        "sql/secrets/rotation/directory_material.sql",
        request.account().to_string(),
        request.source_version(),
        remaining
    )
    .fetch_all(&mut *tx)
    .await?;
    count += u64::try_from(rows.len())?;
    for row in rows {
        let context = crate::secrets::directory::DirectoryContext {
            account: row.account_id.parse()?,
            provider: row.provider.parse()?,
            material_id: row.material_id.parse()?,
            material_revision: row.material_revision,
        };
        let stored = crate::secrets::envelope::DirectoryEnvelope::parse(&row.encrypted_envelope)?;
        let source =
            keys::load_version(&mut *tx, wrapping, context.account, stored.key_version()).await?;
        let material = crate::secrets::directory::open(&source, &context, &stored)?;
        let encrypted = crate::secrets::directory::seal(&target, &context, &material)?.encode();
        sqlx::query_file!(
            "sql/secrets/rotation/directory_update.sql",
            row.account_id,
            row.provider,
            encrypted
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    metrics::counter!("clacks_secret_values_reencrypted_total").increment(count);
    Ok(count)
}
