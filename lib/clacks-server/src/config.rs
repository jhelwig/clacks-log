//! Deployment configuration, with secrets loaded only by their owning operation.

use anyhow::Context;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use url::Url;

/// Deployment-key configuration for encrypting account encryption keys
/// stored in PostgreSQL.
#[derive(Clone, Deserialize)]
pub struct WrappingKeyConfig {
    /// ID of the deployment key used to encrypt newly generated account keys.
    pub active: String,
    /// Raw 32-byte key files indexed by the IDs recorded with encrypted account keys.
    pub files: BTreeMap<String, PathBuf>,
}

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
        /// Standard token authentication method; defaults to Basic with a secret,
        /// or no authentication when no secret is configured.
        #[serde(default)]
        token_endpoint_auth_method: Option<openidconnect::core::CoreClientAuthMethod>,
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
    /// Explicit local development HTTP permission; production defaults to HTTPS.
    #[serde(default)]
    pub allow_http: bool,
    /// Enable standard OpenTelemetry exporters in addition to structured stderr.
    #[serde(default)]
    pub otlp: bool,
    /// Maximum seconds to drain requests after shutdown is requested.
    #[serde(default = "default_shutdown_seconds")]
    pub shutdown_seconds: u64,
}

/// Default deployment request-drain bound.
fn default_shutdown_seconds() -> u64 {
    30
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
            self.shutdown_seconds > 0,
            "shutdown_seconds must be positive"
        );
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
        anyhow::ensure!(self.pending_seconds > 0, "pending_seconds must be positive");
        for (key, provider) in &self.providers {
            anyhow::ensure!(
                !key.is_empty()
                    && key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "provider keys must contain only letters, digits, dash or underscore"
            );
            let issuer = Url::parse(provider.issuer.as_str())?;
            anyhow::ensure!(
                issuer.scheme() == "https" || (self.allow_http && issuer.scheme() == "http"),
                "provider {key} requires HTTPS (or explicit development allow_http)"
            );
            anyhow::ensure!(
                issuer.username().is_empty()
                    && issuer.password().is_none()
                    && issuer.query().is_none()
                    && issuer.fragment().is_none(),
                "provider {key} issuer must not contain credentials, query or fragment"
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

/// Server startup arguments; deployment configuration remains separate from secrets.
#[derive(clap::Parser)]
#[command(
    name = "clacks-server",
    about = "Clacks Log account and synchronization server"
)]
pub struct ServerArgs {
    /// Deployment configuration file.
    #[arg(long, default_value = "clacks-server.toml")]
    pub config: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::ServerConfig;

    #[test]
    fn plaintext_provider_requires_explicit_development_permission() -> anyhow::Result<()> {
        let mut config: ServerConfig = serde_json::from_value(serde_json::json!({
            "public_url": "https://clacks.example",
            "providers": {"primary": {
                "issuer": "http://identity.example",
                "label": "OIDC Provider Name", "mode": "preconfigured", "client_id": "clacks"
            }}
        }))?;
        assert!(config.validate().is_err());
        config.allow_http = true;
        config.validate()?;
        Ok(())
    }
}
