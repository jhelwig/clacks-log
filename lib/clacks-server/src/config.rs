//! Deployment configuration, with secrets loaded only by their owning operation.

use anyhow::Context;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use url::Url;

/// One deployment-configured provider and its client-registration source.
#[derive(Clone, Deserialize)]
pub struct ProviderConfig {
    /// Exact OpenID issuer used for discovery and identity verification.
    pub issuer: openidconnect::IssuerUrl,
    /// Operator-configured label shown in provider selection.
    pub label: String,
    /// Preconfigured client credentials or durable dynamic registration.
    #[serde(flatten)]
    pub registration: RegistrationMode,
}

/// Client credentials provisioned outside ordinary user sign-in.
#[derive(Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RegistrationMode {
    /// Use an existing provider client registration.
    Preconfigured {
        /// Provider-issued client identifier.
        client_id: String,
        /// Optional confidential-client secret file.
        client_secret_file: Option<PathBuf>,
    },
    /// Reuse a registration created by explicit deployment setup.
    Dynamic {
        /// Protected durable registration destination.
        registration_file: PathBuf,
        /// Optional authorization required by the registration endpoint.
        initial_access_token_file: Option<PathBuf>,
    },
}

/// Shared nonsecret deployment inputs; optional runtime paths are not read by maintenance.
#[derive(Clone, Deserialize)]
pub struct ServerConfig {
    /// Public HTTPS origin used for callbacks and mutation-origin checks.
    pub public_url: Url,
    /// Restricted application listener behind the deployment HTTPS boundary.
    #[serde(default = "default_listen")]
    pub listen: SocketAddr,
    /// Runtime database URL file, required only when serving requests.
    pub database_url_file: Option<PathBuf>,
    /// Durable middleware cookie-key file, required only when serving requests.
    pub cookie_key_file: Option<PathBuf>,
    /// Deployment-configured providers keyed by local route identifier.
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
    /// Authoritative sliding browser-login lifetime in days.
    #[serde(default = "default_session_days")]
    pub session_days: i64,
    /// Maximum age of one pending provider round trip in seconds.
    #[serde(default = "default_pending_seconds")]
    pub pending_seconds: i64,
    /// Maximum retained pending round trips per browser session.
    #[serde(default = "default_pending_limit")]
    pub pending_limit: usize,
    /// Explicit local development HTTP permission; production defaults to HTTPS.
    #[serde(default)]
    pub allow_http: bool,
}

/// Default restricted listener for a local reverse-proxy deployment.
fn default_listen() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 3000))
}
/// Default sliding duration selected by account policy.
fn default_session_days() -> i64 {
    14
}
/// Default expiry for one interrupted browser authorization step.
fn default_pending_seconds() -> i64 {
    600
}
/// Default bound on unrelated simultaneous provider round trips.
fn default_pending_limit() -> usize {
    8
}

impl ServerConfig {
    /// Resolve file/environment configuration without reading operation-specific secrets.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let config: Self = config::Config::builder()
            .add_source(config::File::from(path))
            .add_source(
                config::Environment::with_prefix("CLACKS")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;
        config.validate()?;
        Ok(config)
    }

    /// Check shared configuration before provider setup or runtime composition.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.public_url.scheme() == "https"
                || (self.allow_http && self.public_url.scheme() == "http"),
            "public_url requires HTTPS (or explicit development allow_http)"
        );
        anyhow::ensure!(
            self.public_url.host_str().is_some()
                && self.public_url.username().is_empty()
                && self.public_url.password().is_none(),
            "public_url must identify an origin without credentials"
        );
        anyhow::ensure!(
            self.public_url.path() == "/"
                && self.public_url.query().is_none()
                && self.public_url.fragment().is_none(),
            "public_url must be an origin without a path, query or fragment"
        );
        anyhow::ensure!(
            self.session_days > 0 && self.session_days.checked_mul(86400).is_some(),
            "session_days must be a positive representable duration"
        );
        anyhow::ensure!(
            self.pending_seconds > 0 && self.pending_limit > 0,
            "pending flow settings must be positive"
        );
        for key in self.providers.keys() {
            anyhow::ensure!(
                !key.is_empty()
                    && key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "provider keys must contain only letters, digits, dash or underscore"
            );
        }
        Ok(())
    }

    /// Build the fixed callback for a configured provider, never an incoming host header.
    pub fn callback(&self, key: &str) -> anyhow::Result<Url> {
        anyhow::ensure!(
            self.providers.contains_key(key),
            "provider is not configured"
        );
        Ok(self.public_url.join(&format!("auth/callback/{key}"))?)
    }
}

/// Read a deployment secret, removing only a file's final newline characters.
pub(crate) fn secret(path: &Path) -> anyhow::Result<String> {
    let text =
        std::fs::read_to_string(path).context("could not read the configured secret file")?;
    let value = text.trim_end_matches(['\r', '\n']);
    anyhow::ensure!(!value.is_empty(), "configured secret file is empty");
    Ok(value.to_owned())
}
