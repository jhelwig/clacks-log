//! Direct database authorization and Huskarl proof verification for native resources.

use super::error::ClientAuthError;
use crate::{enrollment::AuthorizedClient, runtime::ServerState};
use axum::http::{HeaderMap, Method, Uri};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use huskarl_crypto_native::NativeVerifierPlatform;
use huskarl_resource_server::validator::dpop_proof::DPoPProofValidator;
use jiff_sqlx::ToSqlx;
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};

/// Construct the application's accepted proof algorithms and request-age limits.
pub(crate) fn validator() -> DPoPProofValidator {
    DPoPProofValidator::builder()
        .jws_verifier_platform(Arc::new(NativeVerifierPlatform))
        .allowed_signing_algorithms(vec!["ES256".into()])
        .max_proof_age(Duration::from_secs(60))
        .clock_leeway(Duration::from_secs(5))
        .build()
}

/// Require a current admitted actor and a single fresh proof for this public resource URI.
#[tracing::instrument(skip_all, name = "client.authorize")]
pub(crate) async fn authorize(
    state: &ServerState,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
) -> Result<AuthorizedClient, ClientAuthError> {
    if headers.get_all("authorization").iter().count() != 1
        || headers.get_all("dpop").iter().count() != 1
    {
        return Err(ClientAuthError::Invalid);
    }
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .ok_or(ClientAuthError::Invalid)?;
    let (scheme, token) = authorization
        .split_once(' ')
        .ok_or(ClientAuthError::Invalid)?;
    if !scheme.eq_ignore_ascii_case("DPoP")
        || token.is_empty()
        || token.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(ClientAuthError::Invalid);
    }
    let client = state
        .issuer
        .store()
        .authorized(token)
        .await
        .map_err(unavailable)?
        .ok_or(ClientAuthError::Invalid)?;
    if !client
        .scope
        .to_string()
        .split_whitespace()
        .any(|scope| scope == "sync")
    {
        return Err(ClientAuthError::Scope);
    }

    let proof = headers
        .get("dpop")
        .and_then(|value| value.to_str().ok())
        .ok_or(ClientAuthError::Invalid)?;
    let verified = state
        .proof_validator
        .validate(proof)
        .await
        .map_err(|_| ClientAuthError::Invalid)?;
    if verified.thumbprint.as_deref() != Some(client.key_thumbprint.as_str())
        || verified.htm.as_deref() != Some(method.as_str())
    {
        return Err(ClientAuthError::Invalid);
    }
    let supplied = verified.htu.as_deref().ok_or(ClientAuthError::Invalid)?;
    if supplied.contains(['?', '#']) {
        return Err(ClientAuthError::Invalid);
    }
    let target = format!(
        "{}{}",
        state.config.public_url.as_str().trim_end_matches('/'),
        uri.path()
    );
    let supplied = fluent_uri::Uri::parse(supplied)
        .map_err(|_| ClientAuthError::Invalid)?
        .normalize()
        .into_string();
    let target = fluent_uri::Uri::parse(target.as_str())
        .map_err(|_| ClientAuthError::Invalid)?
        .normalize()
        .into_string();
    let hash = URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()));
    if supplied != target || verified.ath.as_deref() != Some(hash.as_str()) {
        return Err(ClientAuthError::Invalid);
    }

    let jti = verified.jti.ok_or(ClientAuthError::Invalid)?;
    let until = jiff::Timestamp::now() + jiff::SignedDuration::from_secs(70);
    let claimed = sqlx::query_file!(
        "sql/oauth/resource_claim.sql",
        client.key_thumbprint,
        jti,
        until.to_sqlx() as _
    )
    .fetch_optional(&state.store.pool)
    .await
    .map_err(|error| unavailable(error.into()))?
    .is_some();
    if !claimed {
        return Err(ClientAuthError::Invalid);
    }
    tracing::debug!(account_id=%client.account, actor_id=%client.actor, grant_family=%client.family, "Native request authorized");
    Ok(client)
}

/// Report authoritative storage failure without interpreting it as an invalid credential.
fn unavailable(error: anyhow::Error) -> ClientAuthError {
    tracing::error!(reason=%error, "Client authorization storage unavailable");
    ClientAuthError::Unavailable
}

/// Recheck the access, grant and actor bound to an admitted connection before its next operation.
///
/// The initial HTTP upgrade still verifies a fresh proof. This lookup prevents an open
/// socket from retaining authority after ordinary token/grant/client invalidation.
pub(crate) async fn check_current(
    connection: &mut sqlx::PgConnection,
    client: &AuthorizedClient,
) -> Result<(), ClientAuthError> {
    let owner = client.account.to_string();
    let row = sqlx::query_file_as!(
        super::rows::AccessRow,
        "sql/oauth/connection_authority.sql",
        client.access_digest.as_slice(),
        owner
    )
    .fetch_optional(connection)
    .await
    .map_err(|error| unavailable(error.into()))?
    .ok_or(ClientAuthError::Invalid)?;
    let grant = super::grant::GrantState::from_columns(
        &row.family_state,
        row.actor_id.as_deref(),
        row.provisional_until.map(jiff_sqlx::Timestamp::to_jiff),
    )
    .map_err(unavailable)?;
    if grant.active_actor(row.closed_at.map(jiff_sqlx::Timestamp::to_jiff)) != Some(client.actor)
        || row.family_id != client.family
        || row.key_thumbprint != client.key_thumbprint
        || row.expires_at.to_jiff() <= jiff::Timestamp::now()
        || client.expires_at <= jiff::Timestamp::now()
    {
        return Err(ClientAuthError::Invalid);
    }
    if !row.scope.split_whitespace().any(|scope| scope == "sync") {
        return Err(ClientAuthError::Scope);
    }
    Ok(())
}
