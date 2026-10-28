//! Resolve verified provider identities to stable independent accounts.

use super::{
    AccountStore,
    logins::{EstablishedLogin, LoginReference},
};
use crate::auth::{oidc::VerifiedIdentity, principal::Principal};
use clacks_model::ids::AccountId;
use time::Duration;
use ulid::Ulid;

/// Binding fields selected together from account-scoped persistence operations.
pub(super) struct BindingRow {
    pub account_id: String,
    pub identity_id: String,
}

impl BindingRow {
    /// Interpret stored identifiers for an authenticated account operation.
    pub(super) fn principal(self) -> anyhow::Result<Principal> {
        Ok(Principal {
            account_id: self.account_id.parse()?,
            identity_id: self.identity_id.parse()?,
        })
    }
}

impl AccountStore {
    /// Admit a verified sign-in, creating an account only for an unbound identity.
    #[tracing::instrument(skip_all, name = "account.login.admit")]
    pub(crate) async fn provision_login(
        &self,
        identity: VerifiedIdentity,
        lifetime: Duration,
    ) -> anyhow::Result<EstablishedLogin> {
        anyhow::ensure!(
            lifetime.whole_seconds() > 0,
            "login lifetime must be positive"
        );
        for _ in 0..3 {
            let mut transaction = self.pool.begin().await?;
            let found = sqlx::query_file_as!(
                BindingRow,
                "sql/identity_find.sql",
                identity.issuer,
                identity.subject
            )
            .fetch_optional(&mut *transaction)
            .await?;
            let principal = if let Some(binding) = found {
                sqlx::query_file!("sql/account_lock.sql", binding.account_id)
                    .fetch_one(&mut *transaction)
                    .await?;
                let current = sqlx::query_file_as!(
                    BindingRow,
                    "sql/identity_find.sql",
                    identity.issuer,
                    identity.subject
                )
                .fetch_optional(&mut *transaction)
                .await?;
                match current {
                    Some(current) if current.account_id == binding.account_id => {
                        current.principal()?
                    }
                    _ => {
                        transaction.rollback().await?;
                        continue;
                    }
                }
            } else {
                let account_id = AccountId::new();
                let identity_id = Ulid::generate();
                sqlx::query_file!("sql/account_insert.sql", account_id.to_string())
                    .execute(&mut *transaction)
                    .await?;
                let insert = sqlx::query_file!(
                    "sql/identity_insert.sql",
                    identity_id.to_string(),
                    account_id.to_string(),
                    identity.issuer,
                    identity.subject,
                    identity.display_name
                )
                .execute(&mut *transaction)
                .await;
                if let Err(error) = insert {
                    let collision = matches!(&error, sqlx::Error::Database(database) if database.constraint() == Some("identity_issuer_subject"));
                    transaction.rollback().await?;
                    if collision {
                        continue;
                    }
                    return Err(error.into());
                }
                Principal {
                    account_id,
                    identity_id,
                }
            };
            let reference = LoginReference::generate();
            sqlx::query_file!(
                "sql/login_insert.sql",
                reference.secret(),
                principal.account_id.to_string(),
                principal.identity_id.to_string(),
                lifetime.whole_seconds()
            )
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            tracing::info!(account_id = %principal.account_id, identity_id = %principal.identity_id, "Browser login established");
            return Ok(EstablishedLogin {
                principal,
                reference,
            });
        }
        anyhow::bail!("identity binding changed during sign-in; restart sign-in")
    }
}
