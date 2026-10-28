//! PostgreSQL account operations and their transaction boundaries.

use sqlx::PgPool;

pub(crate) mod accounts;
pub(crate) mod logins;

#[cfg(test)]
mod tests;

/// Concrete PostgreSQL access shared by account requests and maintenance work.
#[derive(Clone)]
pub struct AccountStore {
    pool: PgPool,
}

impl AccountStore {
    /// Use an existing role-appropriate pool; this does not migrate its database.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
