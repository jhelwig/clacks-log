//! Point-in-time working contacts and effective facts for desktop file export.

use super::projection::ContactFacts;
use crate::logbooks::selection::SelectedView;
use clacks_model::{
    contact::Qso,
    export::ProfileSet,
    group::DefinitionSet,
    ids::QsoId,
    query::Query,
    reference::ReferenceAssociation,
    scalar::UtcSecond,
    service::{destination::Service, evidence::ImportProvenance},
};
use std::collections::BTreeSet;

/// Saved working contacts to capture for an export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportScope {
    /// Explicit selected identities independently of view and filter.
    Selected(BTreeSet<QsoId>),
    /// Every working contact in a saved view that matches its applied expression.
    Matching {
        /// Current logbook or built-in view.
        view: SelectedView,
        /// Applied history expression.
        filter: Query,
    },
    /// All active working contacts in the account.
    All,
}

/// Counts for the three scopes in one read snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportCounts {
    /// Active explicitly selected identities.
    pub selected: u64,
    /// Active explicitly selected identities outside the view or expression.
    pub hidden: u64,
    /// Active contacts matching the expression in the current view.
    pub matching: u64,
    /// Active contacts across the account.
    pub all: u64,
}

/// A retained date reported by a source for an applicable positive confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmationDate {
    /// The logbook service whose confirmation was reported.
    pub service: Service,
    /// The timestamp supplied with the confirmation.
    pub date: UtcSecond,
    /// Source information for identifying the event the date describes.
    pub provenance: ImportProvenance,
}

/// One saved working contact and its eligible read-side export facts.
#[derive(Clone, Debug)]
pub struct ExportContact {
    /// The complete recorded contact, with unpublished local edits.
    pub qso: Qso,
    /// Effective station values and applicable service status.
    pub facts: ContactFacts,
    /// Location-bearing references from the chosen source plus recorded participation.
    pub contacted_references: BTreeSet<ReferenceAssociation>,
    /// Dates retained by current positive confirmation evidence.
    pub confirmation_dates: Vec<ConfirmationDate>,
}

/// Owned inputs from one account snapshot, independent of later writes.
#[derive(Debug)]
pub struct ExportInput {
    /// Active working contacts in stable identity order.
    pub contacts: Vec<ExportContact>,
    /// Current account reference definitions.
    pub definitions: DefinitionSet,
    /// Current account output profiles.
    pub profiles: ProfileSet,
}
