//! Maintained external associations and exact-version processing decisions.

use crate::ids::{AssociationId, DispositionId, ObservationId, QsoId, SuppressionId};
use crate::service::destination::ExternalRecordKey;
use serde::{Deserialize, Serialize};

/// Lifecycle of a remote identity binding, including retained historical suppression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssociationState {
    /// Established current relationship.
    Active,
    /// Explicitly replaced historical relationship; not automatically reactivated.
    Superseded,
    /// Known local contact deletion; downloads must not offer it as new.
    DeletedQso,
}

/// Unique maintained binding of an external record to at most one local contact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAssociation {
    /// Stable binding identity.
    pub id: AssociationId,
    /// Exact destination-qualified remote identity.
    pub external: ExternalRecordKey,
    /// Bound local contact, including a retained deleted identity.
    pub qso: QsoId,
    /// Current lifecycle state.
    pub state: AssociationState,
}

/// Processing choice for a particular observation occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingDecision {
    /// Durable incoming work requiring explicit operator selection or add/reject.
    PendingReview,
    /// Bound to an existing selected/matched contact.
    Associated,
    /// Explicitly added as a validated local contact.
    Added,
    /// Operator rejected this reviewed version only.
    Rejected,
    /// Suppressed by retained local deletion knowledge.
    Suppressed,
    /// Existing effects remain represented while ordering evidence alone advances.
    CompletedEquivalent,
}

/// Durable review/processing state; accepted effects are carried by synchronization support.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessingDisposition {
    /// Stable processing state identity.
    pub id: DispositionId,
    /// Exact retained observation reviewed.
    pub observation: ObservationId,
    /// Chosen action or outstanding work.
    pub decision: ProcessingDecision,
    /// Resulting binding when processing uses one.
    pub association: Option<AssociationId>,
}

/// Deletion memory independent of download progress and version-specific rejection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletionSuppression {
    /// Stable suppression identity.
    pub id: SuppressionId,
    /// Known deleted contact.
    pub qso: QsoId,
    /// Remote identity which must not be offered as a new contact.
    pub external: ExternalRecordKey,
}
