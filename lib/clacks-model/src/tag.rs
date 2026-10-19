//! Logbook-tag upload policies and pure deletion classification for account-owned contacts.

use crate::{Qso, QsoId, TagId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A tag’s automatic-upload contribution for one external service.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomaticUploadPolicy {
    /// An automatic-upload grant.
    Enable,
    /// An automatic-upload prohibition.
    Forbid,
    /// A neutral contribution; the default policy value.
    #[default]
    Neither,
}

/// Independent tag policies for Logbook of The World (LoTW) and QRZ.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomaticUploadPolicies {
    /// Automatic-upload contribution for LoTW.
    pub lotw: AutomaticUploadPolicy,
    /// Automatic-upload contribution for QRZ.
    pub qrz: AutomaticUploadPolicy,
}

/// An account-owned logbook view defined by contact tag membership.
///
/// Names need not be unique. Tags do not own contact records or synchronization scopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogbookTag {
    /// Stable tag identity, independent of name.
    pub id: TagId,
    /// User-facing logbook name; duplicates are allowed.
    pub name: String,
    /// Per-service policies evaluated across all tags on each contact.
    pub automatic_upload: AutomaticUploadPolicies,
}

impl LogbookTag {
    /// Create a tag with both service policies set to `Neither`.
    ///
    /// Supply a fresh ID for a new tag. This does not enforce name uniqueness or
    /// persist the tag; both duplicate names and untagged contacts are supported.
    pub fn new(id: TagId, name: String) -> Self {
        Self {
            id,
            name,
            automatic_upload: AutomaticUploadPolicies::default(),
        }
    }
}

/// Combine one service’s policies across every tag on a contact.
///
/// Any `Forbid` wins; otherwise at least one `Enable` grants eligibility.
/// Empty input and all-`Neither` input deny automatic uploads. This does not
/// check routing, credentials, service state or manual-upload permission.
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

/// Operator-selected treatment of contacts associated with a tag being deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagDeletionChoice {
    /// Preserve associated contacts and remove only the deleted tag.
    LeaveAll,
    /// Mark every associated contact deleted, including those with other tags.
    DeleteAll,
    /// Delete contacts unique to this tag; preserve shared contacts without this membership.
    DeleteUnique,
}

/// Identity-based contact classification for applying one tag-deletion choice.
///
/// This plan is not a mutation, tombstone, transaction or synchronization result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagDeletionPlan {
    /// All contact identities associated with the selected tag.
    pub affected: BTreeSet<QsoId>,
    /// Contacts to mark deleted, rather than merely making them untagged.
    pub mark_deleted: BTreeSet<QsoId>,
    /// Preserved contacts from which only the selected tag is removed.
    pub remove_membership: BTreeSet<QsoId>,
    /// Number of affected identities that also belong to another tag.
    pub also_tagged_elsewhere: usize,
}

/// Classify contact effects before deleting a tag, without changing any input.
///
/// Supply a consistent snapshot with one working version per contact identity.
/// Use the result for deletion preview and application; delete the tag itself in
/// every choice. The caller owns stale-state revalidation, transactions, actual
/// deletion markers and external-record retention.
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
