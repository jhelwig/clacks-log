//! Output context and profile selection independent of contact selection or file publication.

use crate::{GroupId, OutputProfileId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod rules;

pub use rules::*;

/// The requested output profile and optional program/contest context.
///
/// This does not choose contacts or add participation; callers supply the contact set separately.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportContext {
    /// Output profile governing ordinary versus targeted reference mappings.
    pub output_profile_id: OutputProfileId,
    /// Optional group resolving contest context; required for targeted profiles.
    pub target_group_id: Option<GroupId>,
}

/// Whether a profile uses ordinary interchange mappings or a selected group’s submission plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputPurpose {
    /// Use ordinary group mappings with one output entry per contact and no reference expansion.
    OrdinaryInterchange,
    /// Use the selected group’s profile-specific mapping, partitioning and expansion plan.
    Targeted,
}

/// Reference-output policy, separate from full interchange formatting and publication.
///
/// Common contact-field formatting and program submission validation belong to adapters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputProfile {
    /// Stable output-profile identity used in targeted group mappings.
    pub id: OutputProfileId,
    /// User-facing profile name.
    pub name: String,
    /// Ordinary interchange or group-targeted submission behavior.
    pub purpose: OutputPurpose,
    /// Fields whose competing group values require explicit context selection or omission.
    pub contextual_fields: BTreeSet<String>,
}

/// Output profiles keyed by their own `OutputProfile::id`.
///
/// Callers must keep registry keys consistent with embedded identities.
pub type ProfileSet = BTreeMap<OutputProfileId, OutputProfile>;

mod select;

pub use select::*;

mod evaluate;

pub use evaluate::*;
