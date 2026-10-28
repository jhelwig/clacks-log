//! Temporary standard OpenID request context in middleware-owned session data.

use axum_session_sqlx::SessionPgSession;
use openidconnect::{CsrfToken, Nonce, PkceCodeVerifier};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Middleware key for the small collection of independent authorization requests.
const PENDING: &str = "oidc.pending";

/// Maximum number of pending provider round trips retained per browser session.
const MAXIMUM_PENDING: usize = 8;

/// Purpose bound to one validated callback, not chosen by callback query data.
#[derive(Serialize, Deserialize)]
pub(crate) enum Purpose {
    /// Resolve or provision an account through ordinary sign-in.
    SignIn,
}

/// OIDC request values needed to validate one provider round trip.
#[derive(Serialize, Deserialize)]
pub(crate) struct PendingFlow {
    /// Expected provider configuration key.
    pub provider: String,
    /// Random state compared with the returning browser value.
    pub state: CsrfToken,
    /// Random nonce required in the provider identity token.
    pub nonce: Nonce,
    /// PKCE verifier retained only on the server.
    pub verifier: PkceCodeVerifier,
    /// Creation time for this round trip, in Unix seconds.
    pub created_at: i64,
    /// Account operation authorized by this callback.
    pub purpose: Purpose,
}

/// Retain a request in the standard session, bounding expired/old contexts.
pub(crate) fn insert(session: &SessionPgSession, flow: PendingFlow, age: i64) {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut flows: BTreeMap<String, PendingFlow> = session.get(PENDING).unwrap_or_default();
    flows.retain(|_, value| value.created_at <= now && now.saturating_sub(value.created_at) <= age);
    while flows.len() >= MAXIMUM_PENDING {
        let oldest = flows
            .iter()
            .min_by_key(|(_, value)| value.created_at)
            .map(|(key, _)| key.clone());
        if let Some(key) = oldest {
            flows.remove(&key);
        } else {
            break;
        }
    }
    flows.insert(flow.state.secret().clone(), flow);
    session.set(PENDING, flows);
}

/// Remove and validate a returning request using native middleware session values.
pub(crate) fn take(
    session: &SessionPgSession,
    state: &str,
    provider: &str,
    age: i64,
) -> anyhow::Result<PendingFlow> {
    let mut flows: BTreeMap<String, PendingFlow> = session.get_remove(PENDING).unwrap_or_default();
    let found = flows.remove(state);
    session.set(PENDING, flows);
    let flow = found
        .ok_or_else(|| anyhow::anyhow!("authorization context is missing; restart the action"))?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    anyhow::ensure!(
        flow.state == CsrfToken::new(state.to_owned())
            && flow.provider == provider
            && flow.created_at <= now
            && now.saturating_sub(flow.created_at) <= age,
        "authorization context is invalid or expired; restart the action"
    );
    Ok(flow)
}
