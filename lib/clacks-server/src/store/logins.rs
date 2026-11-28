//! Authoritative browser login validity, independent of middleware data.

use super::{AccountStore, accounts::BindingRow};
use crate::auth::principal::Principal;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jiff::SignedDuration;
use rand::{RngCore, rngs::OsRng};

/// Secret server-side reference to one authoritative browser login.
pub(crate) struct LoginReference(String);

impl LoginReference {
    /// Resolve the reference retained in middleware data, without authorizing it.
    pub(crate) fn from_session(value: String) -> Self {
        Self(value)
    }

    /// Generate a new opaque login reference with 256 random bits.
    pub(super) fn generate() -> Self {
        let mut bytes = [0; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Borrow the reference only for persistence and middleware integration.
    pub(crate) fn secret(&self) -> &str {
        &self.0
    }
}

impl AccountStore {
    /// Refresh an existing unexpired login; missing or invalidated logins stay absent.
    pub(crate) async fn authenticate(
        &self,
        reference: &LoginReference,
        lifetime: SignedDuration,
    ) -> anyhow::Result<Option<Principal>> {
        anyhow::ensure!(lifetime.as_secs() > 0, "login lifetime must be positive");
        sqlx::query_file_as!(
            BindingRow,
            "sql/login_refresh.sql",
            reference.secret(),
            lifetime.as_secs()
        )
        .fetch_optional(&self.pool)
        .await?
        .map(BindingRow::principal)
        .transpose()
    }

    /// Invalidate one authoritative login without changing other account logins.
    #[tracing::instrument(skip_all, name = "account.login.logout")]
    pub(crate) async fn logout(&self, reference: &LoginReference) -> anyhow::Result<()> {
        let result = sqlx::query_file!("sql/login_delete.sql", reference.secret())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() > 0 {
            tracing::info!("Browser login invalidated");
        }
        Ok(())
    }
}
