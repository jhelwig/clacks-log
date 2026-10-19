use crate::{Qso, QsoId, TagId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomaticUploadPolicy {
    Enable,
    Forbid,
    #[default]
    Neither,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomaticUploadPolicies {
    pub lotw: AutomaticUploadPolicy,
    pub qrz: AutomaticUploadPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogbookTag {
    pub id: TagId,
    pub name: String,
    pub automatic_upload: AutomaticUploadPolicies,
}

impl LogbookTag {
    pub fn new(id: TagId, name: String) -> Self {
        Self {
            id,
            name,
            automatic_upload: AutomaticUploadPolicies::default(),
        }
    }
}

pub fn automatic_upload_allowed(policies: impl IntoIterator<Item = AutomaticUploadPolicy>) -> bool {
    let mut enabled = false;
    for policy in policies {
        match policy {
            AutomaticUploadPolicy::Forbid => return false,
            AutomaticUploadPolicy::Enable => enabled = true,
            AutomaticUploadPolicy::Neither => {}
        }
    }
    enabled
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagDeletionChoice {
    LeaveAll,
    DeleteAll,
    DeleteUnique,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagDeletionPlan {
    pub affected: BTreeSet<QsoId>,
    pub mark_deleted: BTreeSet<QsoId>,
    pub remove_membership: BTreeSet<QsoId>,
    pub also_tagged_elsewhere: usize,
}

/// Classifies a consistent input snapshot; callers own transactions and stale-state checks.
pub fn plan_tag_deletion(tag: TagId, choice: TagDeletionChoice, qsos: &[Qso]) -> TagDeletionPlan {
    let affected: BTreeSet<_> = qsos
        .iter()
        .filter(|q| q.tags.contains(&tag))
        .map(|q| q.id)
        .collect();
    let shared: BTreeSet<_> = qsos
        .iter()
        .filter(|q| q.tags.contains(&tag) && q.tags.len() > 1)
        .map(|q| q.id)
        .collect();
    let mark_deleted = match choice {
        TagDeletionChoice::LeaveAll => BTreeSet::new(),
        TagDeletionChoice::DeleteAll => affected.clone(),
        TagDeletionChoice::DeleteUnique => affected.difference(&shared).copied().collect(),
    };
    TagDeletionPlan {
        remove_membership: affected.difference(&mark_deleted).copied().collect(),
        affected,
        mark_deleted,
        also_tagged_elsewhere: shared.len(),
    }
}
