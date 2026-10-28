//! Server state shared by ordinary Axum handlers and the Leptos application.

use crate::{config::ServerConfig, store::AccountStore};
use anyhow::Context;
use axum_session::{Key, SessionConfig, SessionLayer};
use axum_session_sqlx::{SessionPgPool, SessionPgSessionStore};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use time::Duration;

/// Configured application state; construction never executes schema changes.
#[derive(Clone)]
pub struct ServerState {
    /// Shared nonsecret deployment configuration.
    pub config: Arc<ServerConfig>,
    /// Concrete account persistence under the runtime role.
    pub store: AccountStore,
    /// Middleware layer using the existing PostgreSQL session table.
    pub(crate) sessions: SessionLayer<SessionPgPool>,
    /// Bounded provider HTTP client with redirects disabled.
    pub(crate) provider_http: openidconnect::reqwest::Client,
}

impl ServerState {
    /// Load runtime secrets and connect to an already migrated database.
    pub async fn connect(config: ServerConfig) -> anyhow::Result<Self> {
        config.validate()?;
        let database = config
            .database_url_file
            .as_deref()
            .context("runtime database_url_file is required")?;
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&crate::config::secret(database)?)
            .await?;
        let key_file = config
            .cookie_key_file
            .as_deref()
            .context("runtime cookie_key_file is required")?;
        let bytes = std::fs::read(key_file).context("could not read browser-cookie key")?;
        let key = Key::try_from(bytes.as_slice())
            .map_err(|_| anyhow::anyhow!("browser-cookie key requires at least 64 bytes"))?;
        let duration = Duration::seconds(
            config
                .session_days
                .checked_mul(86400)
                .context("session duration is out of range")?,
        );
        let settings = SessionConfig::new()
            .with_table_name("clacks.browser_sessions")
            .with_key(key)
            .with_secure(!config.allow_http)
            .with_http_only(true)
            .with_cookie_path("/")
            .with_lifetime(duration)
            .with_max_lifetime(duration)
            .with_max_age(Some(duration));
        let mut sessions = SessionPgSessionStore::new(None, settings).await?;
        sessions.client = Some(pool.clone().into());
        // Validate credentials locally; provider availability does not gate existing logins.
        for (key, provider) in &config.providers {
            crate::auth::oidc::credentials(provider, &config.callback(key)?)?;
        }
        Ok(Self {
            config: Arc::new(config),
            store: AccountStore::new(pool),
            sessions: SessionLayer::new(sessions),
            provider_http: crate::maintenance::registration::http_client()?,
        })
    }

    /// Selected authoritative sliding lifetime for account operations.
    pub(crate) fn lifetime(&self) -> Duration {
        Duration::days(self.config.session_days)
    }
}

/// Serve the composed production routes until the deployment requests shutdown.
pub async fn serve(state: ServerState, pages: axum::Router<ServerState>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(state.config.listen).await?;
    tracing::info!(listen = %listener.local_addr()?, "Clacks server listening");
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let shutdown = async move {
        #[cfg(unix)]
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        tracing::info!("Server shutdown requested");
    };
    axum::serve(listener, crate::http::router(state.clone(), pages))
        .with_graceful_shutdown(shutdown)
        .await?;
    state.store.close().await;
    tracing::info!("Server requests drained");
    Ok(())
}
