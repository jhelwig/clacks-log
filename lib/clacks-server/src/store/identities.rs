//! Account-scoped identity changes coordinated by the owning account row.

use super::{AccountStore, accounts::BindingRow};
use crate::auth::{oidc::VerifiedIdentity, principal::Principal};
use clacks_model::ids::AccountId;
use sqlx::{Postgres, Transaction};
use ulid::Ulid;

/// A linked identity shown only to its authenticated account owner.
#[derive(Clone)]
pub struct IdentityView {
    /// Stable binding ID used when selecting a specific identity.
    pub identity_id: Ulid,
    /// Exact provider issuer.
    pub issuer: String,
    /// Exact provider subject, not inferred from email.
    pub subject: String,
    /// Optional recorded display label supplied by the provider.
    pub display_name: Option<String>,
}

/// Identity values selected by the account-scoped list query.
struct IdentityRow {
    identity_id: String,
    issuer: String,
    subject: String,
    display_name: Option<String>,
}

/// Current login and authorizing identity bound to one account operation.
pub(crate) struct IdentityAction {
    /// Account that must be locked before changing its identity bindings.
    pub account_id: AccountId,
    /// Original browser login, rechecked at the mutation boundary.
    pub originating_login: String,
    /// Recently verified identity that must remain linked.
    pub authorizing_identity: Ulid,
}

/// Refusals that can be safely explained to the account owner.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// An identity already belongs to a different Clacks account.
    #[error("That login identity belongs to another Clacks account.")]
    OwnedElsewhere,
    /// A selected identity does not belong to the authenticated account.
    #[error("The selected identity is not linked to this account.")]
    WrongAccount,
    /// Removing the target would leave the account without a login identity.
    #[error("The last login identity cannot be removed.")]
    LastIdentity,
    /// The authorizing identity cannot be the identity being removed.
    #[error("Authenticate through an identity that will remain linked.")]
    AuthorizerRemoved,
    /// The initiating login or authorizing binding is no longer valid.
    #[error("The account login changed. Sign in and restart the action.")]
    LoginChanged,
}

/// Whether an identity or role mutation changed durable state.
#[derive(Debug, PartialEq, Eq)]
pub enum ChangeOutcome {
    /// The requested state was already present.
    Unchanged,
    /// The requested mutation committed.
    Changed,
}

impl AccountStore {
    /// List identities belonging to the request's authenticated account.
    pub async fn identities(&self, principal: &Principal) -> anyhow::Result<Vec<IdentityView>> {
        let rows = sqlx::query_file_as!(
            IdentityRow,
            "sql/identity_list.sql",
            principal.account_id.to_string()
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(IdentityView {
                    identity_id: row.identity_id.parse()?,
                    issuer: row.issuer,
                    subject: row.subject,
                    display_name: row.display_name,
                })
            })
            .collect()
    }

    /// Lock the account and recheck the original login and authorizing binding.
    async fn identity_transaction(
        &self,
        action: &IdentityAction,
    ) -> anyhow::Result<Transaction<'static, Postgres>> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query_file!("sql/account_lock.sql", action.account_id.to_string())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(IdentityError::WrongAccount)?;
        sqlx::query_file!(
            "sql/identity_action_validate.sql",
            action.account_id.to_string(),
            action.originating_login,
            action.authorizing_identity.to_string()
        )
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(IdentityError::LoginChanged)?;
        Ok(transaction)
    }

    /// Add an independently verified identity without changing the current login.
    #[tracing::instrument(skip_all, name = "account.identity.link")]
    pub(crate) async fn link_identity(
        &self,
        action: &IdentityAction,
        identity: VerifiedIdentity,
    ) -> anyhow::Result<ChangeOutcome> {
        let mut transaction = self.identity_transaction(action).await?;
        let outcome = bind(&mut transaction, action.account_id, identity).await?;
        transaction.commit().await?;
        if outcome == ChangeOutcome::Changed {
            tracing::info!(account_id = %action.account_id, "Login identity linked");
            metrics::counter!("clacks_identity_changes_total", "kind" => "link").increment(1);
        }
        Ok(outcome)
    }

    /// Remove only an owned identity while retaining a valid authorizing identity.
    #[tracing::instrument(skip_all, name = "account.identity.unlink")]
    pub(crate) async fn unlink_identity(
        &self,
        action: &IdentityAction,
        target: Ulid,
    ) -> anyhow::Result<ChangeOutcome> {
        let mut transaction = self.identity_transaction(action).await?;
        let identities = sqlx::query_file_as!(
            IdentityRow,
            "sql/identity_list.sql",
            action.account_id.to_string()
        )
        .fetch_all(&mut *transaction)
        .await?;
        anyhow::ensure!(
            identities
                .iter()
                .any(|row| row.identity_id == target.to_string()),
            IdentityError::WrongAccount
        );
        anyhow::ensure!(identities.len() > 1, IdentityError::LastIdentity);
        anyhow::ensure!(
            action.authorizing_identity != target,
            IdentityError::AuthorizerRemoved
        );
        sqlx::query_file!(
            "sql/identity_delete.sql",
            action.account_id.to_string(),
            target.to_string()
        )
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        tracing::info!(account_id = %action.account_id, identity_id = %target, "Login identity unlinked");
        metrics::counter!("clacks_identity_changes_total", "kind" => "unlink").increment(1);
        Ok(ChangeOutcome::Changed)
    }

    /// Attach an operator-verified replacement identity, preserving existing logins.
    #[tracing::instrument(skip_all, name = "account.identity.recover", fields(%account_id))]
    pub(crate) async fn recover_identity(
        &self,
        account_id: AccountId,
        identity: VerifiedIdentity,
    ) -> anyhow::Result<ChangeOutcome> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query_file!("sql/account_lock.sql", account_id.to_string())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(IdentityError::WrongAccount)?;
        let outcome = bind(&mut transaction, account_id, identity).await?;
        transaction.commit().await?;
        if outcome == ChangeOutcome::Changed {
            tracing::info!(%account_id, "Account recovery identity linked");
            metrics::counter!("clacks_identity_changes_total", "kind" => "recovery").increment(1);
        }
        Ok(outcome)
    }
}

/// Bind a verified identity within an already locked account transaction.
async fn bind(
    transaction: &mut Transaction<'_, Postgres>,
    account_id: AccountId,
    identity: VerifiedIdentity,
) -> anyhow::Result<ChangeOutcome> {
    if let Some(binding) = sqlx::query_file_as!(
        BindingRow,
        "sql/identity_find.sql",
        identity.issuer,
        identity.subject
    )
    .fetch_optional(&mut **transaction)
    .await?
    {
        anyhow::ensure!(
            binding.account_id == account_id.to_string(),
            IdentityError::OwnedElsewhere
        );
        return Ok(ChangeOutcome::Unchanged);
    }
    let result = sqlx::query_file!(
        "sql/identity_insert.sql",
        Ulid::generate().to_string(),
        account_id.to_string(),
        identity.issuer,
        identity.subject,
        identity.display_name
    )
    .execute(&mut **transaction)
    .await;
    match result {
        Err(sqlx::Error::Database(error))
            if error.constraint() == Some("identity_issuer_subject") =>
        {
            Err(IdentityError::OwnedElsewhere.into())
        }
        Err(error) => Err(error.into()),
        Ok(_) => Ok(ChangeOutcome::Changed),
    }
}
