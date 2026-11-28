//! Server state shared by ordinary Axum handlers and the Leptos application.

use crate::{config::ServerConfig, store::AccountStore};
use anyhow::Context;
use axum_session::{Key, SessionConfig, SessionLayer};
use axum_session_sqlx::{SessionPgPool, SessionPgSessionStore};
use jiff::SignedDuration;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

/// Configured application state; construction never executes schema changes.
#[derive(Clone)]
pub struct ServerState {
    /// Shared protected account-wrapping material for concrete secret consumers.
    pub(crate) wrapping_keys: Arc<crate::secrets::keys::WrappingKeys>,
    /// Stop network waits while allowing admitted storage transactions to finish.
    pub(crate) sync_stop: tokio_util::sync::CancellationToken,
    /// Standard lifetime tracking for upgraded connections outside HTTP draining.
    pub(crate) sync_tasks: tokio_util::task::TaskTracker,
    /// Shared nonsecret deployment configuration.
    pub config: Arc<ServerConfig>,
    /// Concrete account persistence under the runtime role.
    pub store: AccountStore,
    /// Middleware layer using the existing PostgreSQL session table.
    pub(crate) sessions: SessionLayer<SessionPgPool>,
    /// Bounded provider HTTP client with redirects disabled.
    pub(crate) provider_http: openidconnect::reqwest::Client,
    /// Embedded issuer over the same authoritative runtime database.
    pub(crate) issuer: Arc<oauth_as::server::AuthorizationServer<crate::oauth::store::OAuthStore>>,
    /// Standard token/revocation wire handling.
    pub(crate) oauth_http: Arc<
        oauth_as::http::AuthorizationService<
            crate::oauth::store::OAuthStore,
            oauth_as::server::SystemClock,
        >,
    >,
    /// Application proof-age and signature policy for native resources.
    pub(crate) proof_validator:
        Arc<huskarl_resource_server::validator::dpop_proof::DPoPProofValidator>,
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
        let duration = time::Duration::seconds(
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
        let keys = Arc::new(crate::secrets::keys::WrappingKeys::load(
            config
                .wrapping_keys
                .as_ref()
                .context("runtime wrapping_keys configuration is required")?,
        )?);
        let store = AccountStore::new(pool);
        let issuer = Arc::new(crate::oauth::config::issuer(
            crate::oauth::store::OAuthStore::new(store.clone(), keys.clone()),
            &config.public_url,
        ));
        let oauth_http = Arc::new(oauth_as::http::ServiceBuilder::new(issuer.clone()).build()?);

        Ok(Self {
            wrapping_keys: keys,
            sync_stop: tokio_util::sync::CancellationToken::new(),
            sync_tasks: tokio_util::task::TaskTracker::new(),
            config: Arc::new(config),
            store,
            sessions: SessionLayer::new(sessions),
            provider_http: crate::maintenance::registration::http_client()?,
            issuer,
            oauth_http,
            proof_validator: Arc::new(crate::oauth::resource::validator()),
        })
    }

    /// Selected authoritative sliding lifetime for account operations.
    pub(crate) fn lifetime(&self) -> SignedDuration {
        SignedDuration::from_hours(self.config.session_days * 24)
    }
}

/// Serve the composed production routes until the deployment requests shutdown.
pub async fn serve(state: ServerState, pages: axum::Router<ServerState>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(state.config.listen).await?;
    tracing::info!(listen = %listener.local_addr()?, "Clacks server listening");
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let (requested, notified) = tokio::sync::oneshot::channel();
    let sync_stop = state.sync_stop.clone();
    let shutdown = async move {
        #[cfg(unix)]
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        tracing::info!("Server shutdown requested");
        sync_stop.cancel();
        let _ = requested.send(());
    };

    let mut maintenance = tokio::task::JoinSet::new();
    maintenance.spawn(crate::oauth::cleanup::run(
        state.store.pool.clone(),
        state.sync_stop.clone(),
    ));
    maintenance.spawn(crate::retention::run(
        state.store.clone(),
        state.config.journal_entries,
        state.sync_stop.clone(),
    ));

    use std::future::IntoFuture;
    let serving = axum::serve(listener, crate::http::router(state.clone(), pages)?)
        .with_graceful_shutdown(shutdown)
        .into_future();
    tokio::pin!(serving);
    let result: anyhow::Result<()> = async {
        tokio::select! {
            result = &mut serving => result?,
            _ = notified => {
                match tokio::time::timeout(std::time::Duration::from_secs(state.config.shutdown_seconds), &mut serving).await {
                    Ok(result) => result?,
                    Err(_) => {
                        tracing::warn!("Request drain deadline reached");
                        anyhow::bail!("request drain deadline reached");
                    }
                }
            }
        }
        Ok(())
    }.await;

    state.sync_stop.cancel();
    tokio::time::timeout(
        std::time::Duration::from_secs(state.config.shutdown_seconds),
        async {
            while let Some(result) = maintenance.join_next().await {
                result.context("maintenance task failed")?;
            }
            Ok::<(), anyhow::Error>(())
        },
    )
    .await
    .context("maintenance drain deadline reached")??;

    state.sync_tasks.close();
    tokio::time::timeout(
        std::time::Duration::from_secs(state.config.shutdown_seconds),
        state.sync_tasks.wait(),
    )
    .await
    .context("synchronization drain deadline reached")?;
    result?;
    state.store.close().await;
    tracing::info!("Server requests drained");
    Ok(())
}
