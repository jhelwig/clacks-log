//! Browser-safe data and authenticated account actions.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

pub mod clients;
pub mod directory;

/// A deployment-configured sign-in choice without client credentials.
#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderChoice {
    /// Stable deployment key used for this choice.
    pub key: String,
    /// Human-readable provider label.
    pub label: String,
}

/// An account owner's linked login identity.
#[derive(Clone, Serialize, Deserialize)]
pub struct Identity {
    /// Binding used when selecting an identity for an action.
    pub id: String,
    /// Exact issuer displayed for identification.
    pub issuer: String,
    /// Provider subject displayed independently of names or email.
    pub subject: String,
}

/// Account information safe to send to its authenticated browser.
#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    /// Stable Clacks account identity.
    pub id: String,
    /// Identity authenticating this browser.
    pub current_identity: String,
    /// Current server-administrator membership.
    pub administrator: bool,
    /// Identities belonging to this account.
    pub identities: Vec<Identity>,
    /// Configured providers offered for linking.
    pub providers: Vec<ProviderChoice>,
}

/// List deployment-approved sign-in choices.
#[server]
pub async fn providers() -> Result<Vec<ProviderChoice>, ServerFnError> {
    let state = state()?;
    Ok(state
        .config
        .providers
        .iter()
        .map(|(key, provider)| ProviderChoice {
            key: key.clone(),
            label: provider.label.clone(),
        })
        .collect())
}

/// Load the requesting browser's account; no account ID is accepted from the client.
#[server]
pub async fn load_account() -> Result<Account, ServerFnError> {
    let state = state()?;
    let principal = principal(&state).await?;
    let identities = state
        .store
        .identities(&principal)
        .await
        .map_err(clacks_server::auth::error::AuthError::from_store)?;
    let administrator = state
        .store
        .is_administrator(&principal)
        .await
        .map_err(clacks_server::auth::error::AuthError::from_store)?;
    Ok(Account {
        id: principal.account_id.to_string(),
        current_identity: principal.identity_id.to_string(),
        administrator,
        identities: identities
            .into_iter()
            .map(|identity| Identity {
                id: identity.identity_id.to_string(),
                issuer: identity.issuer,
                subject: identity.subject,
            })
            .collect(),
        providers: providers().await?,
    })
}

/// Begin a standard provider sign-in through the browser's session.
#[server]
pub async fn start_sign_in(
    provider: String,
    enrollment: Option<String>,
) -> Result<(), ServerFnError> {
    let state = state()?;
    let session = leptos_axum::extract::<axum_session_sqlx::SessionPgSession>().await?;
    let url = clacks_server::auth::browser::sign_in(
        &state,
        &session,
        provider,
        enrollment.filter(|value| !value.is_empty()),
    )
    .await?;
    leptos_axum::redirect(url.as_str());
    Ok(())
}

/// Start linking through a selected existing identity before adding a new one.
#[server]
pub async fn start_link(
    provider: String,
    authorizing_identity: String,
) -> Result<(), ServerFnError> {
    let state = state()?;
    let session = leptos_axum::extract::<axum_session_sqlx::SessionPgSession>().await?;
    let principal = principal(&state).await?;
    let url = clacks_server::auth::changes::request_link(
        &state,
        &session,
        &principal,
        clacks_server::auth::changes::LinkForm {
            provider,
            authorizing_identity,
        },
    )
    .await?;
    leptos_axum::redirect(url.as_str());
    Ok(())
}

/// Start removal after authenticating through an identity that remains linked.
#[server]
pub async fn start_unlink(
    target: String,
    authorizing_identity: String,
) -> Result<(), ServerFnError> {
    let state = state()?;
    let session = leptos_axum::extract::<axum_session_sqlx::SessionPgSession>().await?;
    let principal = principal(&state).await?;
    let url = clacks_server::auth::changes::request_unlink(
        &state,
        &session,
        &principal,
        clacks_server::auth::changes::UnlinkForm {
            target,
            authorizing_identity,
        },
    )
    .await?;
    leptos_axum::redirect(url.as_str());
    Ok(())
}

/// End this browser's login without logging out other devices or the provider.
#[server]
pub async fn sign_out() -> Result<(), ServerFnError> {
    let state = state()?;
    let session = leptos_axum::extract::<axum_session_sqlx::SessionPgSession>().await?;
    clacks_server::auth::browser::sign_out(&state, &session).await?;
    leptos_axum::redirect("/sign-in");
    Ok(())
}

/// Resolve server composition supplied by the standard Axum integration.
#[cfg(feature = "ssr")]
fn state() -> Result<clacks_server::runtime::ServerState, ServerFnError> {
    use_context().ok_or_else(|| ServerFnError::new("Account service unavailable."))
}

/// Redirect expired browser logins while preserving availability failures as errors.
#[cfg(feature = "ssr")]
async fn principal(
    state: &clacks_server::runtime::ServerState,
) -> Result<clacks_server::auth::principal::Principal, ServerFnError> {
    use axum::extract::FromRequestParts;
    use clacks_server::auth::{error::AuthError, principal::Principal};

    let mut parts = use_context::<axum::http::request::Parts>()
        .ok_or_else(|| ServerFnError::new("Account request context is unavailable."))?;
    match Principal::from_request_parts(&mut parts, state).await {
        Ok(principal) => Ok(principal),
        Err(error) => {
            if matches!(error, AuthError::SignInRequired) {
                leptos_axum::redirect("/sign-in");
            }
            Err(ServerFnError::new(error.to_string()))
        }
    }
}
