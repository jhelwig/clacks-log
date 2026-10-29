//! The native application's supported issuer profile.

use super::store::OAuthStore;
use oauth_as::server::{AuthorizationServer, RefreshRotation, ServerConfig};
use std::time::Duration;
use url::Url;

/// Compose the standard issuer with opaque bound tokens and persistent refresh.
pub(crate) fn issuer(store: OAuthStore, origin: &Url) -> AuthorizationServer<OAuthStore> {
    let origin = origin.as_str().trim_end_matches('/');
    let mut config = ServerConfig::new(origin, format!("{origin}/unused-device-verification"));
    config.authorization_endpoint = Some(format!("{origin}/oauth/authorize"));
    config.token_endpoint = Some(format!("{origin}/oauth/token"));
    config.revocation_endpoint = Some(format!("{origin}/oauth/revoke"));
    config.require_dpop = true;
    config.access_token_ttl = Duration::from_secs(300);
    config.refresh_token_ttl = None;
    config.refresh_rotation = RefreshRotation::Reuse;
    config.refresh_retry_window = Duration::ZERO;
    AuthorizationServer::new(config, store)
}
