//! Identity information passed from protocol verification to account admission.

/// External identity claims after successful provider verification.
pub(crate) struct VerifiedIdentity {
    /// Exact provider issuer, without case folding.
    pub issuer: String,
    /// Exact provider subject, independent of display claims.
    pub subject: String,
    /// Optional display label; never used to infer ownership.
    pub display_name: Option<String>,
}

use crate::config::{ProviderConfig, RegistrationMode};
use openidconnect::{
    ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet, EndpointSet, RedirectUrl,
    core::{CoreClient, CoreProviderMetadata},
};
use url::Url;

/// Client endpoint states returned by standard provider discovery.
pub(crate) type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// Resolve the selected provider's existing credentials without network mutation.
pub(crate) fn credentials(
    config: &ProviderConfig,
    callback: &Url,
) -> anyhow::Result<(ClientId, Option<ClientSecret>)> {
    match &config.registration {
        RegistrationMode::Preconfigured {
            client_id,
            client_secret_file,
        } => {
            anyhow::ensure!(!client_id.is_empty(), "configured client_id is empty");
            Ok((
                ClientId::new(client_id.clone()),
                client_secret_file
                    .as_deref()
                    .map(crate::config::secret)
                    .transpose()?
                    .map(ClientSecret::new),
            ))
        }
        RegistrationMode::Dynamic {
            registration_file, ..
        } => {
            let saved =
                crate::maintenance::registration::load(registration_file, config, callback)?;
            Ok((
                saved.response.client_id().clone(),
                saved.response.client_secret().cloned(),
            ))
        }
    }
}

/// Discover the configured provider through the library; never accept a callback-supplied issuer.
pub(crate) async fn discover(
    config: &ProviderConfig,
    callback: &Url,
    http: &openidconnect::reqwest::Client,
) -> anyhow::Result<DiscoveredClient> {
    let (id, secret) = credentials(config, callback)?;
    let metadata = CoreProviderMetadata::discover_async(config.issuer.clone(), http)
        .await
        .map_err(|_| anyhow::anyhow!("configured provider discovery failed"))?;
    Ok(CoreClient::from_provider_metadata(metadata, id, secret)
        .set_redirect_uri(RedirectUrl::new(callback.to_string())?))
}

use crate::{
    auth::{
        error::AuthError,
        pending::{self, PendingFlow, Purpose},
    },
    runtime::ServerState,
};
use axum_session_sqlx::SessionPgSession;
use openidconnect::{
    AuthenticationFlow, CsrfToken, Nonce, PkceCodeChallenge, core::CoreResponseType,
};

/// Start one standard provider request for its server-recorded Clacks purpose.
pub(crate) async fn authorization_url(
    state: &ServerState,
    session: &SessionPgSession,
    key: String,
    purpose: Purpose,
) -> Result<url::Url, AuthError> {
    let provider = state
        .config
        .providers
        .get(&key)
        .ok_or(AuthError::InvalidCallback)?;
    let callback = state
        .config
        .callback(&key)
        .map_err(|_| AuthError::InvalidCallback)?;
    let client = discover(provider, &callback, &state.provider_http)
        .await
        .map_err(|_| AuthError::Provider)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .set_pkce_challenge(challenge);
    if purpose.requires_recent_authentication() {
        request = request.set_max_age(std::time::Duration::from_secs(300));
    }
    if matches!(&purpose, Purpose::CompleteLink { .. }) {
        request = request.add_prompt(openidconnect::core::CoreAuthPrompt::SelectAccount);
    }
    let (url, csrf, nonce) = request.url();
    pending::insert(
        session,
        PendingFlow {
            provider: key,
            state: csrf,
            nonce,
            verifier,
            created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            purpose,
        },
        state.config.pending_seconds,
    );
    Ok(url)
}
