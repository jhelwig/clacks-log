//! oauth-as Storage implementation over the production Clacks relational schema.

use super::{authorization, cleanup, clients, replay, revocation, tokens};
use crate::secrets::keys::WrappingKeys;
use oauth_as::{
    authorization::{AuthorizationCodeRecord, AuthorizationCodeState},
    client::{Client, ClientId},
    device::{DeviceGrant, DeviceGrantState},
    store::{RevocationWindow, Storage, StorageError, WriteOutcome},
    token::{IssuedToken, RefreshTokenRecord},
};
use sqlx::PgPool;
use std::{sync::Arc, time::SystemTime};

/// Database and immutable deployment keys used by every issuer instance.
#[derive(Clone)]
pub(crate) struct OAuthStore {
    /// Shared runtime connection pool.
    pub(crate) pool: PgPool,
    /// Retained wrapping-key versions loaded by runtime composition.
    pub(crate) keys: Arc<WrappingKeys>,
}

impl OAuthStore {
    /// Bind an already migrated runtime pool and protected key configuration.
    pub(crate) fn new(pool: PgPool, keys: Arc<WrappingKeys>) -> Self {
        Self { pool, keys }
    }

    /// Resolve current actor authorization without treating a token as account authority alone.
    pub(crate) async fn authorized(
        &self,
        token: &str,
    ) -> anyhow::Result<Option<crate::enrollment::AuthorizedClient>> {
        tokens::authorized(&self.pool, token).await
    }
}

/// Preserve diagnostics at the handling boundary while returning the library's opaque store error.
fn error(error: anyhow::Error) -> StorageError {
    tracing::error!(event="oauth.storage.failed", reason=%error, "Issuer persistence failed");
    StorageError::new("issuer persistence failed")
}

impl Storage for OAuthStore {
    async fn get_client(&self, id: &ClientId) -> Result<Option<Arc<Client>>, StorageError> {
        clients::get(&self.pool, id).await.map_err(error)
    }
    async fn put_client(&self, client: Client) -> Result<(), StorageError> {
        clients::put(&self.pool, client, None)
            .await
            .map(|_| ())
            .map_err(error)
    }
    async fn compare_and_swap_client(
        &self,
        expected: &Client,
        updated: Client,
    ) -> Result<bool, StorageError> {
        clients::put(&self.pool, updated, Some(expected))
            .await
            .map_err(error)
    }
    async fn delete_client(
        &self,
        id: &ClientId,
        window: RevocationWindow,
    ) -> Result<bool, StorageError> {
        revocation::client(&self.pool, id, window)
            .await
            .map_err(error)
    }
    async fn put_device_grant(&self, _: DeviceGrant) -> Result<(), StorageError> {
        Err(StorageError::new("device authorization is not enabled"))
    }
    async fn get_device_grant(&self, _: &str) -> Result<Option<DeviceGrant>, StorageError> {
        Err(StorageError::new("device authorization is not enabled"))
    }
    async fn find_device_grant_by_user_code(
        &self,
        _: &str,
    ) -> Result<Option<DeviceGrant>, StorageError> {
        Err(StorageError::new("device authorization is not enabled"))
    }
    async fn take_device_grant(&self, _: &str) -> Result<Option<DeviceGrant>, StorageError> {
        Err(StorageError::new("device authorization is not enabled"))
    }
    async fn compare_and_swap_device_grant(
        &self,
        _: &DeviceGrantState,
        _: DeviceGrant,
    ) -> Result<bool, StorageError> {
        Err(StorageError::new("device authorization is not enabled"))
    }
    async fn put_authorization_code(
        &self,
        record: AuthorizationCodeRecord,
    ) -> Result<(), StorageError> {
        authorization::put(self, record).await.map_err(error)
    }
    async fn compare_and_swap_authorization_code(
        &self,
        expected: &AuthorizationCodeState,
        updated: AuthorizationCodeRecord,
    ) -> Result<bool, StorageError> {
        authorization::compare(self, expected, updated)
            .await
            .map_err(error)
    }
    async fn take_authorization_code(
        &self,
        code: &str,
    ) -> Result<Option<AuthorizationCodeRecord>, StorageError> {
        authorization::take(self, code).await.map_err(error)
    }
    async fn put_token(&self, token: IssuedToken) -> Result<WriteOutcome, StorageError> {
        tokens::put_access(self, token).await.map_err(error)
    }
    async fn get_token(&self, token: &str) -> Result<Option<Arc<IssuedToken>>, StorageError> {
        tokens::get_access(&self.pool, token).await.map_err(error)
    }
    async fn delete_token(&self, token: &str) -> Result<(), StorageError> {
        tokens::delete_access(&self.pool, token)
            .await
            .map_err(error)
    }
    async fn put_refresh_token(
        &self,
        record: RefreshTokenRecord,
    ) -> Result<WriteOutcome, StorageError> {
        tokens::put_refresh(self, record).await.map_err(error)
    }
    async fn get_refresh_token(
        &self,
        token: &str,
    ) -> Result<Option<Arc<RefreshTokenRecord>>, StorageError> {
        tokens::get_refresh(&self.pool, token, false)
            .await
            .map(|value| value.map(Arc::new))
            .map_err(error)
    }
    async fn take_refresh_token(
        &self,
        token: &str,
    ) -> Result<Option<RefreshTokenRecord>, StorageError> {
        tokens::get_refresh(&self.pool, token, true)
            .await
            .map_err(error)
    }
    async fn revoke_token_family(
        &self,
        family: &str,
        window: RevocationWindow,
    ) -> Result<u64, StorageError> {
        revocation::family(&self.pool, family, window)
            .await
            .map_err(error)
    }
    async fn claim_replay_id(&self, id: &str, expires: SystemTime) -> Result<bool, StorageError> {
        replay::claim(&self.pool, id, expires).await.map_err(error)
    }
    async fn sweep_expired(&self, now: SystemTime) -> Result<u64, StorageError> {
        cleanup::sweep(&self.pool, now).await.map_err(error)
    }
}
