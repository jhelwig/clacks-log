//! Immutable server-owned public application registration, separate from account grants.

use oauth_as::{
    client::{Client, ClientAuth, ClientId},
    grant::GrantType,
    scope::ScopeSet,
};
use std::sync::Arc;

/// Release-owned native callback allowlist; oauth-as permits request-time port changes.
const DESKTOP_REDIRECT_URIS: [&str; 2] = [
    "http://127.0.0.1:80/oauth/callback",
    "http://[::1]:80/oauth/callback",
];

/// Resolve the release-owned desktop profile; unknown application IDs are not registered.
///
/// No registration state is read or persisted. An error indicates that the built-in
/// scope could not be parsed by the issuer library.
pub(crate) fn get(id: &ClientId) -> anyhow::Result<Option<Arc<Client>>> {
    if id.as_str() != "clacks-desktop" {
        return Ok(None);
    }
    let scopes = ScopeSet::parse("sync")?;
    Ok(Some(Arc::new(Client {
        client_id: id.clone(),
        auth: ClientAuth::Public,
        grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
        redirect_uris: DESKTOP_REDIRECT_URIS
            .into_iter()
            .map(str::to_owned)
            .collect(),
        allowed_scopes: scopes.clone(),
        default_scopes: scopes,
        name: Some("Clacks Log desktop".to_owned()),
        registration: None,
    })))
}
