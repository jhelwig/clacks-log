//! Nonsecret observed outcomes of key-maintenance commands.

use clacks_model::ids::AccountId;

/// Current ciphertext references or committed migration effects by schema family.
#[derive(Debug, Default)]
pub(crate) struct EncryptedCounts {
    /// Protected service values.
    pub credentials: u64,
    /// Encrypted consumed-code receipts.
    pub receipts: u64,
}

/// Whether the requested migration finished or stopped at a safe boundary.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MaintenanceEnd {
    /// No requested source references remain.
    Complete,
    /// The operator requested cancellation; committed work remains intact.
    Interrupted,
}

/// One invocation's account rotation outcome, not a durable job history.
#[derive(Debug)]
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
    pub migrated: EncryptedCounts,
    /// Current rows still referencing the source.
    pub remaining: EncryptedCounts,
}
