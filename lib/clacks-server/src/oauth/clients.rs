//! Server-owned public application registration; it is not a tenant/client grant.

use super::rows::{ClientRow, TextValue};
use anyhow::Context;
use oauth_as::{
    client::{Client, ClientAuth, ClientId},
    grant::GrantType,
    scope::ScopeSet,
};
use sqlx::PgPool;
use std::sync::Arc;

/// Release-owned native callback allowlist; oauth-as permits request-time port changes.
const DESKTOP_REDIRECT_URIS: [&str; 2] = [
    "http://127.0.0.1:80/oauth/callback",
    "http://[::1]:80/oauth/callback",
];

/// Read a public registration without treating its identifier as an account credential.
pub(crate) async fn get(pool: &PgPool, id: &ClientId) -> anyhow::Result<Option<Arc<Client>>> {
    let mut connection = pool.acquire().await?;
    get_in(&mut connection, id).await
}

/// Read one complete registration on the caller's existing connection.
async fn get_in(
    connection: &mut sqlx::PgConnection,
    id: &ClientId,
) -> anyhow::Result<Option<Arc<Client>>> {
    let Some(row) = sqlx::query_file_as!(ClientRow, "sql/oauth/client_select.sql", id.as_str())
        .fetch_optional(&mut *connection)
        .await?
    else {
        return Ok(None);
    };
    let grants = sqlx::query_file_as!(TextValue, "sql/oauth/client_grants.sql", id.as_str())
        .fetch_all(&mut *connection)
        .await?;
    let scopes = sqlx::query_file_as!(TextValue, "sql/oauth/client_scopes.sql", id.as_str())
        .fetch_all(&mut *connection)
        .await?;
    let grant_types = grants
        .into_iter()
        .map(|item| match item.value.as_str() {
            "authorization_code" => Ok(GrantType::AuthorizationCode),
            "refresh_token" => Ok(GrantType::RefreshToken),
            _ => anyhow::bail!("unsupported stored desktop grant"),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let scopes = ScopeSet::parse(
        &scopes
            .into_iter()
            .map(|item| item.value)
            .collect::<Vec<_>>()
            .join(" "),
    )?;
    Ok(Some(Arc::new(Client {
        client_id: ClientId::new(row.client_id),
        auth: ClientAuth::Public,
        grant_types,
        redirect_uris: DESKTOP_REDIRECT_URIS
            .into_iter()
            .map(str::to_owned)
            .collect(),
        allowed_scopes: scopes.clone(),
        default_scopes: scopes,
        name: Some(row.display_name),
        registration: None,
    })))
}

/// Update the supported public application's label, honoring conditional replacement.
pub(crate) async fn put(
    pool: &PgPool,
    client: Client,
    expected: Option<&Client>,
) -> anyhow::Result<bool> {
    anyhow::ensure!(
        client.client_id.as_str() == "clacks-desktop"
            && matches!(client.auth, ClientAuth::Public)
            && client.registration.is_none(),
        "only the server-owned public desktop registration is supported"
    );
    let mut transaction = pool.begin().await?;
    let locked = sqlx::query_file!("sql/oauth/client_lock.sql", client.client_id.as_str())
        .fetch_optional(&mut *transaction)
        .await?;
    if expected.is_some() && locked.is_none() {
        return Ok(false);
    }
    let current = get_in(&mut transaction, &client.client_id)
        .await?
        .context("desktop registration is not provisioned")?;
    if expected.is_some_and(|expected| expected != current.as_ref()) {
        return Ok(false);
    }
    anyhow::ensure!(
        client.grant_types == current.grant_types
            && client.redirect_uris == current.redirect_uris
            && client.allowed_scopes == current.allowed_scopes
            && client.default_scopes == current.default_scopes,
        "desktop registration protocol settings are release-managed"
    );
    let name = client
        .name
        .as_deref()
        .context("desktop registration requires a display name")?;
    sqlx::query_file!("sql/oauth/client_put.sql", client.client_id.as_str(), name)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(true)
}
