//! Browser-held authorization requests and explicit owner approval.

use super::{ApprovalPurpose, RecoveryTarget, remember_approval};
use crate::{
    auth::{error::AuthError, principal::Principal},
    runtime::ServerState,
};
use axum::{
    extract::{FromRequestParts, RawQuery, State},
    http::request::Parts,
    response::Redirect,
};
use axum_session_sqlx::SessionPgSession;
use oauth_as::{authorization::AuthorizationRequest, server::UserApproval};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Session entry containing bounded, independently correlated desktop requests.
const REQUESTS: &str = "desktop.authorization.requests";

/// Validated wire input retained across the existing provider sign-in flow.
#[derive(Serialize, Deserialize)]
struct ApprovalRequest {
    /// Original decoded query pairs, revalidated before issuing any code.
    parameters: Vec<(String, String)>,
    /// User-proposed client label, not account authority.
    label: String,
    /// Existing account and writer selected together for recovery.
    recovery: Option<RecoveryTarget>,
    /// Browser-context creation time, bounded by the normal pending-flow age.
    created_at: i64,
}

/// Browser-safe information describing the explicit approval consequence.
#[derive(Clone, Serialize, Deserialize)]
pub struct ApprovalSummary {
    /// Deployment origin the desktop requested.
    pub server: String,
    /// Verified browser account.
    pub account: String,
    /// Proposed installation label.
    pub label: String,
    /// Exact replacement consequence presented before approval.
    pub consequence: String,
}

/// Encode one correlation value as a safe application-relative return location.
pub fn location(request: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("request", request)
        .finish();
    format!("/account/clients/approve?{query}")
}

/// Retain a validated native request and route through normal sign-in when necessary.
pub(crate) async fn start(
    State(state): State<ServerState>,
    session: SessionPgSession,
    RawQuery(query): RawQuery,
    mut parts: Parts,
) -> Result<Redirect, AuthError> {
    let parameters: Vec<(String, String)> =
        url::form_urlencoded::parse(query.as_deref().unwrap_or_default().as_bytes())
            .into_owned()
            .collect();
    let native = AuthorizationRequest::from_pairs(parameters.clone());
    let validated = state
        .issuer
        .validate_authorization_request(&native)
        .await
        .map_err(|_| AuthError::InvalidCallback)?;
    if validated.client_id.as_str() != "clacks-desktop" || native.dpop_jkt.is_none() {
        return Err(AuthError::InvalidCallback);
    }
    let correlation = native
        .state
        .as_deref()
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .ok_or(AuthError::InvalidCallback)?
        .to_owned();
    let field = |name: &str| -> Result<Option<String>, AuthError> {
        let mut matches = parameters.iter().filter(|(key, _)| key == name);
        let value = matches.next().map(|(_, value)| value.clone());
        if matches.next().is_some() {
            return Err(AuthError::InvalidCallback);
        }
        Ok(value)
    };
    let label = field("client_label")?.ok_or(AuthError::InvalidCallback)?;
    if label.trim().is_empty() || label.chars().count() > 128 {
        return Err(AuthError::InvalidCallback);
    }
    let account = field("clacks_account")?
        .map(|value| value.parse())
        .transpose()
        .map_err(|_| AuthError::InvalidCallback)?;
    let actor = field("clacks_actor")?
        .map(|value| value.parse())
        .transpose()
        .map_err(|_| AuthError::InvalidCallback)?;
    let recovery = match (account, actor) {
        (None, None) => None,
        (Some(account), Some(actor)) => Some(RecoveryTarget { account, actor }),
        _ => return Err(AuthError::InvalidCallback),
    };

    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut requests: BTreeMap<String, ApprovalRequest> = session.get(REQUESTS).unwrap_or_default();
    requests.retain(|_, request| {
        request.created_at <= now
            && now.saturating_sub(request.created_at) <= state.config.pending_seconds
    });
    if requests.len() >= 8 {
        return Err(AuthError::InvalidCallback);
    }
    requests.insert(
        correlation.clone(),
        ApprovalRequest {
            parameters,
            label,
            recovery,
            created_at: now,
        },
    );
    session.set(REQUESTS, requests);

    match Principal::from_request_parts(&mut parts, &state).await {
        Ok(_) => Ok(Redirect::to(&location(&correlation))),
        Err(AuthError::SignInRequired) => {
            let query = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("enrollment", &correlation)
                .finish();
            Ok(Redirect::to(&format!("/sign-in?{query}")))
        }
        Err(error) => Err(error),
    }
}

/// Require a live retained request before starting its provider sign-in.
pub(crate) fn require_pending(
    session: &SessionPgSession,
    request: &str,
    age: i64,
) -> Result<(), AuthError> {
    load(session, request, age).map(|_| ())
}

/// Read only a current request from normal session data.
fn load(session: &SessionPgSession, request: &str, age: i64) -> Result<ApprovalRequest, AuthError> {
    let mut requests: BTreeMap<String, ApprovalRequest> = session.get(REQUESTS).unwrap_or_default();
    let request = requests.remove(request).ok_or(AuthError::InvalidCallback)?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    if request.created_at > now || now.saturating_sub(request.created_at) > age {
        return Err(AuthError::InvalidCallback);
    }
    Ok(request)
}

/// Describe the current account-bound approval without exposing protocol secrets.
pub async fn summary(
    state: &ServerState,
    session: &SessionPgSession,
    principal: &Principal,
    request: &str,
) -> Result<ApprovalSummary, AuthError> {
    let pending = load(session, request, state.config.pending_seconds)?;
    let purpose = super::recovery::purpose(state, principal, pending.recovery).await?;
    let consequence = match purpose {
        ApprovalPurpose::New => {
            "Authorize a new client. The desktop will open an empty workspace for this account."
                .into()
        }
        ApprovalPurpose::RecoverActive { actor } => format!(
            "Replace credentials for client {actor}. Its existing writer identity and pending work are preserved; previous credentials stop working."
        ),
        ApprovalPurpose::ReplaceClosed { actor } => format!(
            "Client {actor} is closed or unknown. Authorize a new writer identity; old unaccepted work requires explicit reconciliation."
        ),
    };
    Ok(ApprovalSummary {
        server: state.config.public_url.to_string(),
        account: principal.account_id.to_string(),
        label: pending.label,
        consequence,
    })
}

/// Approve or deny a retained request using current browser authority and issuer validation.
#[tracing::instrument(skip_all, name="client.approval", fields(account_id=%principal.account_id))]
pub async fn decide(
    state: &ServerState,
    session: &SessionPgSession,
    principal: &Principal,
    request: &str,
    allow: bool,
) -> Result<String, AuthError> {
    let pending = load(session, request, state.config.pending_seconds)?;
    let purpose = super::recovery::purpose(state, principal, pending.recovery).await?;
    let native = AuthorizationRequest::from_pairs(pending.parameters);
    let validated = state
        .issuer
        .validate_authorization_request(&native)
        .await
        .map_err(|_| AuthError::InvalidCallback)?;
    let redirect = if allow {
        let code = state
            .issuer
            .issue_authorization_code(UserApproval::granted(
                &validated,
                principal.account_id.to_string(),
            ))
            .await
            .map_err(|_| AuthError::Unavailable)?;
        remember_approval(
            state.issuer.store(),
            principal,
            &code.code,
            &pending.label,
            purpose,
        )
        .await
        .map_err(AuthError::from_store)?;
        code.location(&validated.redirect_uri)
    } else {
        validated.denied().location()
    };

    let mut requests: BTreeMap<String, ApprovalRequest> = session.get(REQUESTS).unwrap_or_default();
    requests.remove(request);
    session.set(REQUESTS, requests);
    tracing::info!(allowed = allow, "Desktop authorization decision recorded");
    Ok(redirect)
}
