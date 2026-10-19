use crate::{GroupId, OutputProfileId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod rules;

pub use rules::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportContext {
    pub output_profile_id: OutputProfileId,
    pub target_group_id: Option<GroupId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputPurpose {
    OrdinaryInterchange,
    Targeted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputProfile {
    pub id: OutputProfileId,
    pub name: String,
    pub purpose: OutputPurpose,
    pub contextual_fields: BTreeSet<String>,
}

pub type ProfileSet = BTreeMap<OutputProfileId, OutputProfile>;
