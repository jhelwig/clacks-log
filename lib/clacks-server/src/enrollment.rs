//! Browser-approved desktop membership, separate from OAuth application registration.

pub mod approval;
pub(crate) mod membership;
mod recovery;

use crate::{
    auth::principal::Principal,
    oauth::{
        digest::{CredentialKind, digest},
        store::OAuthStore,
    },
};
use clacks_model::ids::{AccountId, ClientActorId};
use oauth_as::{authorization::AuthorizationCodeRecord, scope::ScopeSet};
use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;

/// Verified client authority for one protected resource request.
#[derive(Clone)]
pub(crate) struct AuthorizedClient {
    /// Owning account from durable token/enrollment association.
    pub account: AccountId,
    /// Current admitted writer.
    pub actor: ClientActorId,
    /// Exact OAuth grant family used on this request.
    pub family: String,
    /// Required proof key.
    pub key_thumbprint: String,
    /// Stored granted permissions.
    pub scope: ScopeSet,
    /// Existing one-way access lookup used to recheck this connection's authority.
    pub access_digest: [u8; 32],
    /// Original access expiration; refreshing requires another authenticated upgrade.
    pub expires_at: OffsetDateTime,
}

/// Account and former writer selected together for desktop recovery.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct RecoveryTarget {
    /// Existing account the desktop intends to recover.
    pub account: AccountId,
    /// Existing writer associated with that workspace.
    pub actor: ClientActorId,
}

/// Explicit browser-approved enrollment or credential recovery action.
#[derive(Clone, Serialize, serde::Deserialize)]
pub enum ApprovalPurpose {
    /// Enroll a new installation into an empty account workspace.
    New,
    /// Replace credentials while retaining an authorized writer identity.
    RecoverActive {
        /// Authorized writer whose credentials will be replaced.
        actor: ClientActorId,
    },
    /// Approve a fresh identity for a closed or no-longer-known writer.
    ReplaceClosed {
        /// Former writer retained only as recovery context.
        actor: ClientActorId,
    },
}

impl ApprovalPurpose {
    /// Decode the persisted approval alternative before membership activation.
    pub(crate) fn from_stored(purpose: &str, actor: Option<&str>) -> anyhow::Result<Self> {
        match (purpose, actor) {
            ("new", None) => Ok(Self::New),
            ("recover_active", Some(actor)) => Ok(Self::RecoverActive {
                actor: actor.parse()?,
            }),
            ("replace_closed", Some(actor)) => Ok(Self::ReplaceClosed {
                actor: actor.parse()?,
            }),
            _ => anyhow::bail!("invalid enrollment approval purpose or target"),
        }
    }
}

/// Non-secret account-owned client information for browser management.
#[derive(Clone, Serialize)]
pub struct ClientSummary {
    /// Stable admitted client identity.
    pub actor: ClientActorId,
    /// User-visible installation label.
    pub label: String,
    /// Time of actual enrollment activation.
    pub authorized_at: OffsetDateTime,
    /// Explicit revocation time, absent for an authorized client.
    pub closed_at: Option<OffsetDateTime>,
}

/// Attach immutable account-owner approval before disclosing the code redirect.
pub(crate) async fn remember_approval(
    store: &OAuthStore,
    principal: &Principal,
    code: &str,
    label: &str,
    purpose: ApprovalPurpose,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !label.trim().is_empty() && label.chars().count() <= 128,
        "client label must contain 1–128 characters"
    );
    let account = principal.account_id.to_string();
    let mut transaction = store.pool.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", account)
        .fetch_one(&mut *transaction)
        .await?;
    let valid = sqlx::query_file!(
        "sql/enrollment/principal.sql",
        account,
        principal.identity_id.to_string()
    )
    .fetch_one(&mut *transaction)
    .await?
    .valid;
    anyhow::ensure!(valid, "authorizing login identity is no longer linked");
    let (kind, target) = match purpose {
        ApprovalPurpose::New => ("new", None),
        ApprovalPurpose::RecoverActive { actor } => ("recover_active", Some(actor.to_string())),
        ApprovalPurpose::ReplaceClosed { actor } => ("replace_closed", Some(actor.to_string())),
    };
    if let Some(target) = target.as_deref() {
        let active = sqlx::query_file!("sql/enrollment/client_lock.sql", account, target)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some_and(|client| client.closed_at.is_none());
        anyhow::ensure!(
            active == (kind == "recover_active"),
            "client recovery eligibility changed; review the request again"
        );
    }
    let hash = digest(CredentialKind::Code, code);
    let changed = sqlx::query_file!(
        "sql/oauth/approval_put.sql",
        hash.as_slice(),
        account,
        kind,
        target,
        label.trim()
    )
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    anyhow::ensure!(
        changed == 1,
        "authorization code is not available for this account"
    );
    transaction.commit().await?;
    Ok(())
}

/// Finalize membership only when the library's final code transition can commit.
pub(crate) async fn activate_in(
    connection: &mut PgConnection,
    code: &AuthorizationCodeRecord,
) -> anyhow::Result<Option<ClientActorId>> {
    membership::activate(connection, code).await
}

/// List only the authenticated account's clients; no caller-supplied account grants authority.
pub async fn clients(
    state: &crate::runtime::ServerState,
    principal: &Principal,
) -> anyhow::Result<Vec<ClientSummary>> {
    client_rows(&state.store.pool, principal.account_id).await
}

/// Resolve real client rows without inventing connection or synchronization timestamps.
pub(crate) async fn client_rows(
    pool: &PgPool,
    account: AccountId,
) -> anyhow::Result<Vec<ClientSummary>> {
    membership::list(pool, account).await
}

/// Close one writer and invalidate its grant families in the same guarded transaction.
pub async fn revoke(
    state: &crate::runtime::ServerState,
    principal: &Principal,
    actor: ClientActorId,
) -> anyhow::Result<()> {
    revoke_in_store(&state.store.pool, principal, actor).await?;
    let _ = state.store.changes.send(principal.account_id);
    Ok(())
}

/// Apply explicit client revocation to the durable account-owned membership.
#[tracing::instrument(skip_all, name="account.client.revoke", fields(account_id=%principal.account_id, actor_id=%actor))]
pub(crate) async fn revoke_in_store(
    pool: &PgPool,
    principal: &Principal,
    actor: ClientActorId,
) -> anyhow::Result<()> {
    membership::revoke(pool, principal, actor).await
}
