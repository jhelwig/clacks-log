//! Durable deployment registration using the standard OpenID client library.

use crate::config::{ProviderConfig, RegistrationMode};
use anyhow::Context;
use openidconnect::{
    AccessToken, ClientName, IssuerUrl, RedirectUrl,
    core::{CoreClientRegistrationRequest, CoreClientRegistrationResponse, CoreProviderMetadata},
    registration::EmptyAdditionalClientMetadata,
};
use serde::{Deserialize, Serialize};
use std::io::Write;
use url::Url;

/// Protected provider client registration reused across processes and releases.
#[derive(Serialize, Deserialize)]
pub(crate) struct SavedRegistration {
    /// Exact discovery issuer associated with these credentials.
    pub issuer: IssuerUrl,
    /// Configured callback when this registration was created.
    pub callback: Url,
    /// Standard response, including optional management and expiration metadata.
    pub response: CoreClientRegistrationResponse,
}

/// Outcome of an explicit provider-registration operation.
#[derive(Debug, PartialEq, Eq)]
pub enum RegistrationOutcome {
    /// The expected existing registration was reused without provider mutation.
    Reused,
    /// A new registration was durably published.
    Created,
}

/// Configure an HTTP client without redirects or native-TLS dependencies.
pub(crate) fn http_client() -> anyhow::Result<openidconnect::reqwest::Client> {
    Ok(openidconnect::reqwest::Client::builder()
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}

/// Load a matching, unexpired client registration without contacting its provider.
pub(crate) fn load(
    path: &std::path::Path,
    provider: &ProviderConfig,
    callback: &Url,
) -> anyhow::Result<SavedRegistration> {
    let saved: SavedRegistration = serde_json::from_slice(
        &std::fs::read(path).context("could not read saved provider registration")?,
    )
    .map_err(|_| anyhow::anyhow!("saved provider registration is invalid"))?;
    anyhow::ensure!(
        saved.issuer == provider.issuer && &saved.callback == callback,
        "saved registration does not match configured issuer/callback; operator action required"
    );
    if let Some(expires) = saved.response.client_secret_expires_at() {
        anyhow::ensure!(
            expires.timestamp() == 0
                || expires.timestamp() > time::OffsetDateTime::now_utc().unix_timestamp(),
            "registered client secret has expired; operator action required"
        );
    }
    Ok(saved)
}

/// Register a deployment once or reuse matching saved credentials; no database is opened.
#[tracing::instrument(skip_all, name = "account.provider.register")]
pub async fn run(provider: &ProviderConfig, callback: &Url) -> anyhow::Result<RegistrationOutcome> {
    let RegistrationMode::Dynamic {
        registration_file,
        initial_access_token_file,
    } = &provider.registration
    else {
        anyhow::bail!(
            "provider uses preconfigured credentials; dynamic registration was not selected"
        )
    };
    if registration_file.try_exists()? {
        load(registration_file, provider, callback)?;
        return Ok(RegistrationOutcome::Reused);
    }
    let parent = registration_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    // Establish the protected output before the remote registration can commit.
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let client = http_client()?;
    let metadata = CoreProviderMetadata::discover_async(provider.issuer.clone(), &client)
        .await
        .map_err(|_| anyhow::anyhow!("provider discovery failed during registration"))?;
    let endpoint = metadata
        .registration_endpoint()
        .context("provider has no dynamic registration endpoint")?;
    let token = initial_access_token_file
        .as_deref()
        .map(crate::config::secret)
        .transpose()?
        .map(AccessToken::new);
    let response = CoreClientRegistrationRequest::new(
        vec![RedirectUrl::new(callback.to_string())?],
        EmptyAdditionalClientMetadata {},
    )
        .set_client_name(Some(ClientName::new("Clacks Log".to_owned()).into()))
        .set_initial_access_token(token)
        .register_async(endpoint, &client)
        .await
        .map_err(|error| match error {
            openidconnect::registration::ClientRegistrationError::Response(status, _, _) => anyhow::anyhow!("provider registration returned HTTP {status}; inspect provider diagnostics before retrying"),
            _ => anyhow::anyhow!("provider registration failed; completion may be uncertain, inspect provider diagnostics before retrying"),
        })?;
    let saved = SavedRegistration {
        issuer: provider.issuer.clone(),
        callback: callback.clone(),
        response,
    };
    serde_json::to_writer(temporary.as_file_mut(), &saved)
        .context("registration completed but encoding/saving credentials failed; inspect provider state before retrying")?;
    temporary.flush().context("registration completed but flushing credentials failed; inspect provider state before retrying")?;
    temporary.as_file().sync_all().context("registration completed but syncing credentials failed; inspect provider state before retrying")?;
    if let Err(error) = temporary.persist_noclobber(registration_file) {
        let (_, recovery_path) = error.file.keep().map_err(|_| anyhow::anyhow!("registration completed but saving its credentials failed; inspect provider state before retrying"))?;
        anyhow::bail!(
            "registration completed; credentials retained at {} because publication failed; investigate before retrying",
            recovery_path.display()
        );
    }
    tracing::info!("Provider registration saved");
    Ok(RegistrationOutcome::Created)
}
