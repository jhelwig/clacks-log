//! Nonsecret observed outcomes of key-maintenance commands.

use clacks_model::ids::AccountId;

/// Whether the requested migration finished or stopped at a safe boundary.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) enum MaintenanceEnd {
    /// No requested source references remain.
    Complete,
    /// The operator requested cancellation; committed work remains intact.
    Interrupted,
}

/// One invocation's account rotation outcome, not a durable job history.
#[derive(Debug, serde::Serialize)]
pub(crate) struct AccountRotationReport {
    /// Account being maintained.
    pub account: AccountId,
    /// Source account-key version.
    pub from_version: i64,
    /// Requested active target.
    pub to_version: i64,
    /// Completed migration or safe interruption.
    pub end: MaintenanceEnd,
    /// Rows committed by this invocation.
    pub migrated_receipts: u64,
    /// Current rows still referencing the source.
    pub remaining_receipts: u64,
}

/// Observed effects of one deployment wrapping-key migration invocation.
#[derive(Debug, serde::Serialize)]
pub(crate) struct RewrapReport {
    /// Source deployment key ID.
    pub from: String,
    /// Target deployment key ID.
    pub to: String,
    /// Completion or safe cancellation.
    pub end: MaintenanceEnd,
    /// Account-key envelopes committed by this invocation.
    pub migrated_account_keys: u64,
    /// Current envelopes still requiring the source wrapping key.
    pub remaining_account_keys: u64,
}

/// Failed account rotation with the work already committed by this invocation.
#[derive(Debug, thiserror::Error)]
#[error(
    "account {account} rotation {from_version}→{to_version} stopped after {migrated_receipts} receipts; retry the same source/target"
)]
pub(crate) struct AccountRotationError {
    /// Account being maintained.
    pub account: AccountId,
    /// Requested source account-key version.
    pub from_version: i64,
    /// Requested target account-key version.
    pub to_version: i64,
    /// Rows committed before failure, excluding the rolled-back batch.
    pub migrated_receipts: u64,
    /// Failing operation, including row context when available.
    #[source]
    pub source: anyhow::Error,
}

/// Failed wrapping-key migration with its committed envelope count.
#[derive(Debug, thiserror::Error)]
#[error(
    "rewrap {from}→{to} stopped after {migrated_account_keys} committed account keys; retry the same source/target"
)]
pub(crate) struct RewrapError {
    /// Requested source deployment key ID.
    pub from: String,
    /// Requested target deployment key ID.
    pub to: String,
    /// Account-key envelopes committed before failure.
    pub migrated_account_keys: u64,
    /// Account being processed when failure occurred, if one was selected.
    pub account: Option<AccountId>,
    /// Selected account-key version when failure occurred.
    pub key_version: Option<i64>,
    /// Failing operation, including account/key context when available.
    #[source]
    pub source: anyhow::Error,
}
