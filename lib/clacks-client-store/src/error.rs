//! Storage failures that callers can distinguish from stale edit outcomes.

use clacks_model::ids::AccountId;

/// A workspace cannot be opened or modified under the requested contract.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The file belongs to another application or has inconsistent structural data.
    #[error("invalid Clacks workspace: {detail}")]
    InvalidDatabase {
        /// The failed structural invariant, without bound values or file contents.
        detail: String,
    },
    /// A newer application must handle this database.
    #[error("workspace schema {found} is newer than supported schema {supported}")]
    IncompatibleSchema {
        /// Version read from the workspace.
        found: i64,
        /// Latest embedded migration.
        supported: i64,
    },
    /// The caller selected a workspace belonging to a different account.
    #[error("workspace belongs to account {actual}, not {expected}")]
    WrongAccount {
        /// Caller-selected account.
        expected: AccountId,
        /// Persisted account.
        actual: AccountId,
    },
    /// Enrollment changed since the caller read it.
    #[error("enrollment binding changed; reload it before updating")]
    StaleBinding,
    /// Initial enrollment requires an empty local workspace.
    #[error("export local records and import them into an empty enrolled workspace")]
    PopulatedWorkspace,
}
