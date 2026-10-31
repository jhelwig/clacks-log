//! Native coupled-effect descriptors.

use super::VersionReference;
use clacks_model::ids::OperationId;
use serde::{Deserialize, Serialize};

/// Durable descriptor whose members must be represented before acknowledging its operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoupledRequirement {
    /// Stable atomic-unit identity.
    pub id: OperationId,
    /// Accepted members; later established successors may cover effects.
    pub members: Vec<VersionReference>,
}
