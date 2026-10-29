//! Operator-owned server roles, independent of browser session data.

use super::{
    AccountStore,
    identities::{ChangeOutcome, IdentityError},
};
use crate::auth::principal::Principal;
use clacks_model::ids::AccountId;

impl AccountStore {
    /// Read the current server role for an authenticated request's account.
    pub async fn is_administrator(&self, principal: &Principal) -> anyhow::Result<bool> {
        Ok(sqlx::query_file!(
            "sql/administrator_find.sql",
            principal.account_id.to_string()
        )
        .fetch_one(&self.pool)
        .await?
        .present)
    }

    /// Grant or revoke a role using deployment-controlled maintenance access.
    #[tracing::instrument(skip_all, name = "account.role.change", fields(%account_id, enabled = enabled))]
    pub(crate) async fn set_administrator(
        &self,
        account_id: AccountId,
        enabled: bool,
    ) -> anyhow::Result<ChangeOutcome> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query_file!("sql/account_lock.sql", account_id.to_string())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(IdentityError::WrongAccount)?;
        let result = if enabled {
            sqlx::query_file!("sql/administrator_grant.sql", account_id.to_string())
                .execute(&mut *transaction)
                .await?
        } else {
            sqlx::query_file!("sql/administrator_revoke.sql", account_id.to_string())
                .execute(&mut *transaction)
                .await?
        };
        transaction.commit().await?;
        if result.rows_affected() == 0 {
            return Ok(ChangeOutcome::Unchanged);
        }
        tracing::info!(%account_id, enabled, "Server administrator role changed");
        Ok(ChangeOutcome::Changed)
    }
}
