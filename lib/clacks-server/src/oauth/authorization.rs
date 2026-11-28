//! Single-use authorization codes with encrypted library-compatible replay receipts.

use super::{
    digest::{CredentialKind, digest},
    rows::CodeRow,
    store::OAuthStore,
};
use crate::secrets::{
    envelope::ReceiptEnvelope,
    keys,
    receipt::{self, IssuedReferences, ReceiptContext},
};
use anyhow::Context;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use oauth_as::{
    authorization::{AuthorizationCodeRecord, AuthorizationCodeState},
    client::ClientId,
    scope::ScopeSet,
};
use sqlx::PgConnection;

/// Seal issued references inside the caller's account-locked code transaction.
async fn encrypted(
    store: &OAuthStore,
    connection: &mut PgConnection,
    record: &AuthorizationCodeRecord,
) -> anyhow::Result<Option<ReceiptEnvelope>> {
    let Some((access, refresh)) = record.state.minted() else {
        return Ok(None);
    };
    if access.is_none() && refresh.is_none() {
        return Ok(None);
    }
    let hash = digest(CredentialKind::Code, &record.code);
    let approval = sqlx::query_file!("sql/oauth/approval_select.sql", hash.as_slice())
        .fetch_one(&mut *connection)
        .await?;
    let context = ReceiptContext {
        account: record.subject.parse()?,
        code_digest: hash,
        purpose: approval.purpose,
    };
    let key = keys::active_in(connection, &store.keys, context.account).await?;
    Ok(Some(receipt::seal(
        &key,
        &context,
        &IssuedReferences {
            access_token: access.map(str::to_owned),
            refresh_token: refresh.map(str::to_owned),
        },
    )?))
}

/// Restore the public library record using the presented code and its protected references.
async fn decode(
    store: &OAuthStore,
    connection: &mut PgConnection,
    code: &str,
    row: CodeRow,
) -> anyhow::Result<AuthorizationCodeRecord> {
    let hash = digest(CredentialKind::Code, code);
    let references = match row.receipt.as_deref() {
        None => IssuedReferences {
            access_token: None,
            refresh_token: None,
        },
        Some(value) => {
            let value = ReceiptEnvelope::parse(value)?;
            let approval = sqlx::query_file!("sql/oauth/approval_select.sql", hash.as_slice())
                .fetch_one(&mut *connection)
                .await?;
            let context = ReceiptContext {
                account: row.account_id.parse()?,
                code_digest: hash,
                purpose: approval.purpose,
            };
            let key = keys::load_version(
                &mut *connection,
                &store.keys,
                context.account,
                value.key_version(),
            )
            .await?;
            receipt::open(&key, &context, &value)?
        }
    };
    let mut record = AuthorizationCodeRecord::new(
        code,
        ClientId::new(row.client_id),
        row.redirect_uri,
        ScopeSet::parse(&row.scope)?,
        row.account_id,
        row.code_challenge,
        row.expires_at.to_jiff().try_into()?,
    );
    record.redirect_uri_was_explicit = row.redirect_explicit;
    record.issued_at = row.issued_at.to_jiff().try_into()?;
    record.resource = row.resources;
    record.dpop_jkt = Some(row.key_thumbprint.into());
    record.state = match row.state.as_str() {
        "issued" => AuthorizationCodeState::Issued,
        "consumed" => AuthorizationCodeState::Consumed {
            access_token: references.access_token,
            refresh_token: references.refresh_token,
        },
        "replayed" => AuthorizationCodeState::Replayed {
            access_token: references.access_token,
            refresh_token: references.refresh_token,
        },
        _ => anyhow::bail!("unrecognized authorization code state"),
    };
    Ok(record)
}

/// Persist the complete library code state without removing immutable host approval metadata.
pub(crate) async fn put(store: &OAuthStore, record: AuthorizationCodeRecord) -> anyhow::Result<()> {
    let mut transaction = store.pool.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", record.subject)
        .fetch_one(&mut *transaction)
        .await?;
    let receipt = encrypted(store, &mut transaction, &record).await?;
    put_in(&mut transaction, &record, receipt.as_ref()).await?;
    transaction.commit().await?;
    Ok(())
}

/// Atomically remove a code and return its state, retaining separate host approval context.
pub(crate) async fn take(
    store: &OAuthStore,
    code: &str,
) -> anyhow::Result<Option<AuthorizationCodeRecord>> {
    let hash = digest(CredentialKind::Code, code);
    let Some(owner) = sqlx::query_file!("sql/oauth/code_owner.sql", hash.as_slice())
        .fetch_optional(&store.pool)
        .await?
    else {
        return Ok(None);
    };
    let mut transaction = store.pool.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", owner.account_id)
        .fetch_one(&mut *transaction)
        .await?;
    let Some(row) = sqlx::query_file_as!(CodeRow, "sql/oauth/code_select.sql", hash.as_slice())
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Ok(None);
    };
    let result = decode(store, &mut transaction, code, row).await?;
    sqlx::query_file!("sql/oauth/code_delete.sql", hash.as_slice())
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(Some(result))
}

/// Commit the conditional final code transition and actual enrollment as one operation.
pub(crate) async fn compare(
    store: &OAuthStore,
    expected: &AuthorizationCodeState,
    record: AuthorizationCodeRecord,
) -> anyhow::Result<bool> {
    let account: clacks_model::ids::AccountId = record.subject.parse()?;
    let hash = digest(CredentialKind::Code, &record.code);
    let mut transaction = store.pool.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", record.subject)
        .fetch_one(&mut *transaction)
        .await?;
    let Some(row) = sqlx::query_file_as!(CodeRow, "sql/oauth/code_select.sql", hash.as_slice())
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Ok(false);
    };
    let current = decode(store, &mut transaction, &record.code, row).await?;
    if current.state != *expected {
        return Ok(false);
    }
    let Some(actor) = crate::enrollment::activate_in(&mut transaction, &record).await? else {
        return Ok(false);
    };
    let receipt = encrypted(store, &mut transaction, &record).await?;
    put_in(&mut transaction, &record, receipt.as_ref()).await?;
    transaction.commit().await?;
    let _ = store.changes.send(account);
    metrics::counter!("clacks_client_authorizations_total").increment(1);
    tracing::info!(event="client.authorization.activated", account_id=%record.subject, actor_id=%actor, "Client authorization committed");
    Ok(true)
}

/// Save scalar fields and replace scope/resource children within the caller's transaction.
async fn put_in(
    connection: &mut PgConnection,
    record: &AuthorizationCodeRecord,
    receipt: Option<&ReceiptEnvelope>,
) -> anyhow::Result<()> {
    let hash = digest(CredentialKind::Code, &record.code);
    let key = record
        .dpop_jkt
        .as_deref()
        .context("desktop authorization requires a bound key")?;
    let state = match record.state {
        AuthorizationCodeState::Issued => "issued",
        AuthorizationCodeState::Consumed { .. } => "consumed",
        AuthorizationCodeState::Replayed { .. } => "replayed",
    };
    let issued_at = Timestamp::try_from(record.issued_at)?;
    let expires_at = Timestamp::try_from(record.expires_at)?;
    let receipt = receipt.map(ReceiptEnvelope::encode);
    sqlx::query_file!(
        "sql/oauth/code_put.sql",
        hash.as_slice(),
        record.subject,
        record.client_id.as_str(),
        record.redirect_uri,
        record.redirect_uri_was_explicit,
        record.code_challenge,
        key,
        issued_at.to_sqlx() as _,
        expires_at.to_sqlx() as _,
        state,
        receipt
    )
    .execute(&mut *connection)
    .await?;
    sqlx::query_file!("sql/oauth/code_scopes_clear.sql", hash.as_slice())
        .execute(&mut *connection)
        .await?;
    sqlx::query_file!("sql/oauth/code_resources_clear.sql", hash.as_slice())
        .execute(&mut *connection)
        .await?;
    let scopes: Vec<String> = record
        .scope
        .to_string()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    sqlx::query_file!("sql/oauth/code_scopes_put.sql", hash.as_slice(), &scopes)
        .execute(&mut *connection)
        .await?;
    sqlx::query_file!(
        "sql/oauth/code_resources_put.sql",
        hash.as_slice(),
        &record.resource
    )
    .execute(connection)
    .await?;
    Ok(())
}
