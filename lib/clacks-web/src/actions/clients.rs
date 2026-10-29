//! Browser actions for explicit desktop authorization and account-owned client management.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Client lifecycle information displayed to its account owner.
#[derive(Clone, Serialize, Deserialize)]
pub struct Client {
    /// Stable writer identity.
    pub actor: String,
    /// User-provided installation label.
    pub label: String,
    /// Actual enrollment time.
    pub authorized_at: String,
    /// Actual closure time when explicitly revoked.
    pub closed_at: Option<String>,
}

/// Safe approval content without the retained protocol request.
#[derive(Clone, Serialize, Deserialize)]
pub struct Approval {
    /// Deployment origin receiving authorization.
    pub server: String,
    /// Account authenticated in this browser.
    pub account: String,
    /// Installation label proposed by the desktop.
    pub label: String,
    /// Recovery or new-enrollment consequence requiring explicit approval.
    pub consequence: String,
}

/// List only clients belonging to the currently authenticated account.
#[server]
pub async fn list_clients() -> Result<Vec<Client>, ServerFnError> {
    let state = super::state()?;
    let principal = super::principal(&state).await?;
    Ok(clacks_server::enrollment::clients(&state, &principal)
        .await
        .map_err(clacks_server::auth::error::AuthError::from_store)?
        .into_iter()
        .map(|client| Client {
            actor: client.actor.to_string(),
            label: client.label,
            authorized_at: client.authorized_at.to_string(),
            closed_at: client.closed_at.map(|value| value.to_string()),
        })
        .collect())
}

/// Revoke a selected owned client without deleting shared contacts or other clients.
#[server]
pub async fn revoke_client(actor: String) -> Result<(), ServerFnError> {
    let state = super::state()?;
    let principal = super::principal(&state).await?;
    let actor = actor
        .parse()
        .map_err(|_| ServerFnError::new("Invalid client ID."))?;
    clacks_server::enrollment::revoke(&state, &principal, actor)
        .await
        .map_err(clacks_server::auth::error::AuthError::from_store)?;
    Ok(())
}

/// Load the current retained request with its account-specific approval consequence.
#[server]
pub async fn load_approval(request: String) -> Result<Approval, ServerFnError> {
    let state = super::state()?;
    let principal = super::principal(&state).await?;
    let session = leptos_axum::extract::<axum_session_sqlx::SessionPgSession>().await?;
    let summary =
        clacks_server::enrollment::approval::summary(&state, &session, &principal, &request)
            .await?;
    Ok(Approval {
        server: summary.server,
        account: summary.account,
        label: summary.label,
        consequence: summary.consequence,
    })
}

/// Submit explicit approval or denial and return to the validated native callback.
#[server(prefix = "/oauth", endpoint = "approve")]
pub async fn decide_approval(request: String, decision: String) -> Result<(), ServerFnError> {
    let allow = match decision.as_str() {
        "approve" => true,
        "deny" => false,
        _ => return Err(ServerFnError::new("Choose Approve or Deny.")),
    };
    let state = super::state()?;
    let principal = super::principal(&state).await?;
    let session = leptos_axum::extract::<axum_session_sqlx::SessionPgSession>().await?;
    let redirect =
        clacks_server::enrollment::approval::decide(&state, &session, &principal, &request, allow)
            .await?;
    leptos_axum::redirect(&redirect);
    Ok(())
}
