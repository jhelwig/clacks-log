//! OpenID Connect client setup and verified identity values.

use crate::config::{ProviderConfig, RegistrationMode};
use crate::{
    auth::{
        error::AuthError,
        pending::{self, PendingFlow, Purpose},
    },
    runtime::ServerState,
};
use axum_session_sqlx::SessionPgSession;
use openidconnect::{
    AuthType, ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet, EndpointSet, RedirectUrl,
    core::{CoreClient, CoreClientAuthMethod, CoreProviderMetadata},
};
use openidconnect::{
    AuthenticationFlow, CsrfToken, Nonce, PkceCodeChallenge, core::CoreResponseType,
};
use url::Url;

/// External identity claims after successful provider verification.
pub(crate) struct VerifiedIdentity {
    /// Exact provider issuer, without case folding.
    pub issuer: String,
    /// Exact provider subject, independent of display claims.
    pub subject: String,
    /// Optional display label; never used to infer ownership.
    pub display_name: Option<String>,
}

/// Client endpoint states returned by standard provider discovery.
pub(crate) type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// Client identity, secret and authentication method retained together for setup.
pub(crate) struct ClientCredentials {
    /// Provider-issued application identity.
    id: ClientId,
    /// Confidential-client secret, absent for public clients.
    secret: Option<ClientSecret>,
    /// Native OAuth request authentication selected from registration metadata.
    auth_type: AuthType,
}

impl ClientCredentials {
    /// Accept supported standard methods only when their secret requirements match.
    fn new(
        id: ClientId,
        secret: Option<ClientSecret>,
        method: CoreClientAuthMethod,
    ) -> anyhow::Result<Self> {
        let auth_type = match &method {
            CoreClientAuthMethod::ClientSecretBasic => AuthType::BasicAuth,
            CoreClientAuthMethod::ClientSecretPost | CoreClientAuthMethod::None => {
                AuthType::RequestBody
            }
            _ => anyhow::bail!(
                "unsupported token_endpoint_auth_method; use client_secret_basic, client_secret_post or none; preserve the registration and fix provider configuration"
            ),
        };
        let requires_secret = !matches!(method, CoreClientAuthMethod::None);
        anyhow::ensure!(
            secret.is_some() == requires_secret,
            "token_endpoint_auth_method and client secret are inconsistent; fix provider configuration without automatically registering again"
        );
        Ok(Self {
            id,
            secret,
            auth_type,
        })
    }
}

/// Resolve the selected provider's existing credentials without network mutation.
pub(crate) fn credentials(
    config: &ProviderConfig,
    callback: &Url,
) -> anyhow::Result<ClientCredentials> {
    match &config.registration {
        RegistrationMode::Preconfigured {
            client_id,
            client_secret_file,
            token_endpoint_auth_method,
        } => {
            anyhow::ensure!(!client_id.is_empty(), "configured client_id is empty");
            let secret = client_secret_file
                .as_deref()
                .map(crate::config::secret)
                .transpose()?
                .map(ClientSecret::new);
            let method = token_endpoint_auth_method.clone().unwrap_or_else(|| {
                if secret.is_some() {
                    CoreClientAuthMethod::ClientSecretBasic
                } else {
                    CoreClientAuthMethod::None
                }
            });
            ClientCredentials::new(ClientId::new(client_id.clone()), secret, method)
        }
        RegistrationMode::Dynamic {
            registration_file, ..
        } => {
            let saved =
                crate::maintenance::registration::load(registration_file, config, callback)?;
            ClientCredentials::new(
                saved.response.client_id().clone(),
                saved.response.client_secret().cloned(),
                saved
                    .response
                    .token_endpoint_auth_method()
                    .cloned()
                    .unwrap_or(CoreClientAuthMethod::ClientSecretBasic),
            )
        }
    }
}

/// Discover the configured provider through the library; never accept a callback-supplied issuer.
pub(crate) async fn discover(
    config: &ProviderConfig,
    callback: &Url,
    http: &openidconnect::reqwest::Client,
) -> anyhow::Result<DiscoveredClient> {
    let credentials = credentials(config, callback)?;
    let metadata = CoreProviderMetadata::discover_async(config.issuer.clone(), http)
        .await
        .map_err(|_| anyhow::anyhow!("configured provider discovery failed"))?;
    Ok(
        CoreClient::from_provider_metadata(metadata, credentials.id, credentials.secret)
            .set_auth_type(credentials.auth_type)
            .set_redirect_uri(RedirectUrl::new(callback.to_string())?),
    )
}

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
        .map_err(|error| {
            tracing::warn!(provider = %key, reason = %error, "Identity provider client setup failed");
            AuthError::Provider
        })?;
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
            created_at: jiff::Timestamp::now()
                .as_nanosecond()
                .div_euclid(1_000_000_000) as i64,
            purpose,
        },
        state.config.pending_seconds,
    );
    Ok(url)
}
