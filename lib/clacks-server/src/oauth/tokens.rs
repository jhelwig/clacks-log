//! Hashed credential rows and current actor/family eligibility.

use super::{
    digest::{CredentialKind, digest},
    rows::TokenRow,
    store::OAuthStore,
};
use crate::enrollment::AuthorizedClient;
use anyhow::Context;
use oauth_as::{
    client::ClientId,
    scope::ScopeSet,
    store::WriteOutcome,
    token::{IssuedToken, RefreshTokenRecord, RefreshTokenState},
};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use time::OffsetDateTime;

/// Metadata shared by access and refresh writes for the same grant family.
struct FamilyInput<'a> {
    /// Owning account.
    account: &'a str,
    /// Library-generated grant identity.
    family: &'a str,
    /// Public application registration.
    client: &'a str,
    /// Proof-key thumbprint fixed at approval.
    key: &'a str,
    /// Original grant authorization instant.
    established: OffsetDateTime,
}

/// Check current family/member state and the library barriers within the write transaction.
async fn permit(connection: &mut PgConnection, input: &FamilyInput<'_>) -> anyhow::Result<bool> {
    sqlx::query_file!("sql/account_lock.sql", input.account)
        .fetch_one(&mut *connection)
        .await?;
    if sqlx::query_file!(
        "sql/oauth/barrier_check.sql",
        input.family,
        input.client,
        input.established
    )
    .fetch_one(&mut *connection)
    .await?
    .revoked
    {
        return Ok(false);
    }
    let mut row = sqlx::query_file_as!(
        super::rows::GrantRow,
        "sql/oauth/grant_select.sql",
        input.family
    )
    .fetch_optional(&mut *connection)
    .await?;
    if row.is_none() {
        sqlx::query_file!(
            "sql/oauth/grant_provisional.sql",
            input.account,
            input.family,
            input.client,
            input.established,
            input.key
        )
        .execute(&mut *connection)
        .await?;
        row = sqlx::query_file_as!(
            super::rows::GrantRow,
            "sql/oauth/grant_select.sql",
            input.family
        )
        .fetch_optional(&mut *connection)
        .await?;
    }
    let row = row.context("grant family was not persisted")?;
    anyhow::ensure!(
        row.account_id == input.account
            && row.client_id == input.client
            && row.key_thumbprint == input.key
            && row.grant_established_at == input.established,
        "credential metadata differs from its grant family"
    );
    match row.state.as_str() {
        "provisional" => Ok(row.actor_id.is_none()
            && row
                .provisional_until
                .is_some_and(|until| until > OffsetDateTime::now_utc())),
        "active" => {
            let actor = row.actor_id.context("active grant has no actor")?;
            let member = sqlx::query_file!("sql/enrollment/client_lock.sql", input.account, actor)
                .fetch_optional(connection)
                .await?;
            Ok(member.is_some_and(|member| member.closed_at.is_none()))
        }
        "revoked" => Ok(false),
        _ => anyhow::bail!("invalid grant state"),
    }
}

/// Persist an access token only while its grant remains eligible.
pub(crate) async fn put_access(
    store: &OAuthStore,
    record: IssuedToken,
) -> anyhow::Result<WriteOutcome> {
    let account = record
        .subject
        .as_deref()
        .context("desktop tokens require an account subject")?;
    let family = record
        .family_id
        .as_deref()
        .context("desktop tokens require a grant family")?;
    let key = record
        .jkt
        .as_deref()
        .context("desktop tokens require DPoP binding")?;
    let input = FamilyInput {
        account,
        family,
        client: record.client_id.as_str(),
        key,
        established: record.grant_established_at.into(),
    };
    let mut transaction = store.pool.begin().await?;
    if !permit(&mut transaction, &input).await? {
        return Ok(WriteOutcome::RefusedRevoked);
    }
    let hash = digest(CredentialKind::Access, &record.access_token);
    let issued = OffsetDateTime::from(record.issued_at);
    let expires = OffsetDateTime::from(record.expires_at);
    sqlx::query_file!(
        "sql/oauth/access_put.sql",
        hash.as_slice(),
        account,
        family,
        input.client,
        key,
        issued,
        input.established,
        expires
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query_file!("sql/oauth/access_scopes_clear.sql", hash.as_slice())
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/oauth/access_resources_clear.sql", hash.as_slice())
        .execute(&mut *transaction)
        .await?;
    let scopes: Vec<String> = record
        .scope
        .to_string()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    sqlx::query_file!("sql/oauth/access_scopes_put.sql", hash.as_slice(), &scopes)
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!(
        "sql/oauth/access_resources_put.sql",
        hash.as_slice(),
        &record.resource
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(WriteOutcome::Applied)
}

/// Persist refresh metadata without retaining its usable credential or a response cache.
pub(crate) async fn put_refresh(
    store: &OAuthStore,
    record: RefreshTokenRecord,
) -> anyhow::Result<WriteOutcome> {
    anyhow::ensure!(
        record.retry.is_none(),
        "refresh response caching is not enabled"
    );
    let account = record
        .subject
        .as_deref()
        .context("refresh requires an account subject")?;
    let key = record
        .jkt
        .as_deref()
        .context("refresh requires DPoP binding")?;
    let input = FamilyInput {
        account,
        family: &record.family_id,
        client: record.client_id.as_str(),
        key,
        established: record.grant_established_at.into(),
    };
    let mut transaction = store.pool.begin().await?;
    if !permit(&mut transaction, &input).await? {
        return Ok(WriteOutcome::RefusedRevoked);
    }
    let hash = digest(CredentialKind::Refresh, &record.refresh_token);
    let expires = record.expires_at.map(OffsetDateTime::from);
    let state = match record.state {
        RefreshTokenState::Active => "active",
        RefreshTokenState::Spent => "spent",
    };
    sqlx::query_file!(
        "sql/oauth/refresh_put.sql",
        hash.as_slice(),
        account,
        record.family_id,
        input.client,
        key,
        input.established,
        expires,
        state
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query_file!("sql/oauth/refresh_scopes_clear.sql", hash.as_slice())
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/oauth/refresh_resources_clear.sql", hash.as_slice())
        .execute(&mut *transaction)
        .await?;
    let scopes: Vec<String> = record
        .scope
        .to_string()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    sqlx::query_file!("sql/oauth/refresh_scopes_put.sql", hash.as_slice(), &scopes)
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!(
        "sql/oauth/refresh_resources_put.sql",
        hash.as_slice(),
        &record.resource
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(WriteOutcome::Applied)
}

/// Whether a persisted credential currently names an authorized actor.
fn active(row: &TokenRow) -> bool {
    row.family_state == "active" && row.actor_id.is_some() && row.closed_at.is_none()
}

/// Reconstruct a library access record from the exact credential presented for lookup.
pub(crate) async fn get_access(
    pool: &PgPool,
    token: &str,
) -> anyhow::Result<Option<Arc<IssuedToken>>> {
    let hash = digest(CredentialKind::Access, token);
    let mut transaction = pool.begin().await?;
    let Some(row) = sqlx::query_file_as!(TokenRow, "sql/oauth/access_select.sql", hash.as_slice())
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Ok(None);
    };
    if !active(&row) {
        return Ok(None);
    }
    let mut record = IssuedToken::new(
        token,
        ClientId::new(row.client_id),
        Some(row.account_id),
        ScopeSet::parse(&row.scope)?,
        row.issued_at
            .context("access issuance time is missing")?
            .into(),
        row.expires_at.context("access expiry is missing")?.into(),
    );
    record.grant_established_at = row.grant_established_at.into();
    record.family_id = Some(row.family_id);
    record.jkt = Some(row.key_thumbprint.into());
    record.resource = row.resources;
    transaction.commit().await?;
    Ok(Some(Arc::new(record)))
}

/// Delete one opaque token by the same one-way lookup used to authorize it.
pub(crate) async fn delete_access(pool: &PgPool, token: &str) -> anyhow::Result<()> {
    let hash = digest(CredentialKind::Access, token);
    sqlx::query_file!("sql/oauth/access_delete.sql", hash.as_slice())
        .execute(pool)
        .await?;
    Ok(())
}

/// Read or atomically remove a refresh record while retaining library key-binding checks.
pub(crate) async fn get_refresh(
    pool: &PgPool,
    token: &str,
    take: bool,
) -> anyhow::Result<Option<RefreshTokenRecord>> {
    let hash = digest(CredentialKind::Refresh, token);
    let mut transaction = pool.begin().await?;
    let Some(row) = sqlx::query_file_as!(TokenRow, "sql/oauth/refresh_select.sql", hash.as_slice())
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Ok(None);
    };
    if !active(&row) {
        return Ok(None);
    }
    let mut record = RefreshTokenRecord::new(
        token,
        ClientId::new(row.client_id),
        Some(row.account_id),
        ScopeSet::parse(&row.scope)?,
        row.family_id,
    );
    record.grant_established_at = row.grant_established_at.into();
    record.expires_at = row.expires_at.map(Into::into);
    record.jkt = Some(row.key_thumbprint.into());
    record.resource = row.resources;
    record.state = match row.token_state.as_deref() {
        Some("active") => RefreshTokenState::Active,
        Some("spent") => RefreshTokenState::Spent,
        _ => anyhow::bail!("invalid refresh token state"),
    };
    if take {
        sqlx::query_file!("sql/oauth/refresh_delete.sql", hash.as_slice())
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(Some(record))
}

/// Resolve an unexpired live access token to its account and admitted client.
pub(crate) async fn authorized(
    pool: &PgPool,
    token: &str,
) -> anyhow::Result<Option<AuthorizedClient>> {
    let hash = digest(CredentialKind::Access, token);
    let mut transaction = pool.begin().await?;
    let row = sqlx::query_file_as!(TokenRow, "sql/oauth/access_select.sql", hash.as_slice())
        .fetch_optional(&mut *transaction)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    if !active(&row)
        || !row
            .expires_at
            .is_some_and(|expiry| expiry > OffsetDateTime::now_utc())
    {
        return Ok(None);
    }
    let result = AuthorizedClient {
        account: row.account_id.parse()?,
        actor: row
            .actor_id
            .context("active grant actor missing")?
            .parse()?,
        family: row.family_id,
        key_thumbprint: row.key_thumbprint,
        scope: ScopeSet::parse(&row.scope)?,
    };
    transaction.commit().await?;
    Ok(Some(result))
}
