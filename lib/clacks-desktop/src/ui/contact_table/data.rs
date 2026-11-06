//! Bounded row storage for the full logical extent of a virtual table.

use clacks_client_store::read::contacts::{ContactPage, ContactRow};
use clacks_model::ids::QsoId;
use std::collections::{BTreeMap, BTreeSet};

/// Number of rows in each internal read, independent of the displayed extent.
pub(in crate::ui) const PAGE_SIZE: u32 = 100;

/// Counts from the latest accepted query snapshot, independent of cached pages.
#[derive(Default)]
pub(in crate::ui) struct TableCounts {
    /// Contacts matching the applied expression.
    pub matching_total: u64,
    /// Selected active contacts within the view.
    pub selected_total: u64,
    /// Selected contacts excluded by the applied expression.
    pub selected_outside_filter: u64,
}

/// Only the pages needed by active table viewports, plus their logical counts.
#[derive(Default)]
pub(in crate::ui) struct TableData {
    /// Last accepted counts; absent before the first usable reply.
    pub counts: Option<TableCounts>,
    /// Working rows keyed by their absolute starting position.
    pages: BTreeMap<u32, Vec<ContactRow>>,
}

impl TableData {
    /// Install a reply at its originating offset, replacing old query data when requested.
    pub fn accept(&mut self, offset: u32, page: ContactPage, replace: bool) {
        if replace {
            self.pages.clear();
        }
        self.counts = Some(TableCounts {
            matching_total: page.matching_total,
            selected_total: page.selected_total,
            selected_outside_filter: page.selected_outside_filter,
        });
        self.pages.insert(offset, page.rows);
    }

    /// Discard both rows and counts after a scope change that cannot retain old results.
    pub fn clear(&mut self) {
        self.pages.clear();
        self.counts = None;
    }

    /// Logical row count; Show selected bypasses the expression when choosing rows.
    pub fn len(&self, selected_only: bool) -> usize {
        self.counts.as_ref().map_or(0, |counts| {
            let count = if selected_only {
                counts.selected_total
            } else {
                counts.matching_total
            };
            usize::try_from(count).unwrap_or(usize::MAX)
        })
    }

    /// Return a loaded row at its absolute result index, or a loading placeholder.
    pub fn row(&self, index: usize) -> Option<&ContactRow> {
        let offset = page_offset(index)?;
        self.pages
            .get(&offset)?
            .get(index.checked_sub(offset as usize)?)
    }

    /// Find a loaded contact's absolute index for identity-preserving navigation.
    pub fn index(&self, id: QsoId) -> Option<usize> {
        self.pages.iter().find_map(|(offset, rows)| {
            rows.iter()
                .position(|row| row.working.qso.id() == id)
                .map(|index| *offset as usize + index)
        })
    }

    /// Keep only visible/pending-target pages; return the first missing required offset.
    pub fn require(&mut self, needed: &BTreeSet<u32>) -> Option<u32> {
        self.pages.retain(|offset, _| needed.contains(offset));
        if self.counts.is_none() {
            return Some(0);
        }
        needed
            .iter()
            .copied()
            .find(|offset| !self.pages.contains_key(offset))
    }
}

/// Translate an absolute row index to the existing store's internal page boundary.
pub(super) fn page_offset(index: usize) -> Option<u32> {
    u32::try_from(index)
        .ok()
        .map(|index| index / PAGE_SIZE * PAGE_SIZE)
}
