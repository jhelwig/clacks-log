//! Complete-prefix coverage and position-dependent server-history ancestry.

use crate::{
    error::{MetadataError, UnknownReason},
    record::Position,
};
use clacks_model::ids::{AccountId, TimelineId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Completely installed prefixes, independently certified for each timeline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<(TimelineId,u64)>", into = "Vec<(TimelineId,u64)>")]
pub struct Coverage(BTreeMap<TimelineId, u64>);

impl Coverage {
    /// Check that every retained prefix fits the synchronization counter domain.
    pub fn validate(&self) -> Result<(), MetadataError> {
        for revision in self.0.values() {
            crate::error::validate_counter(*revision, "coverage revision")?;
        }
        Ok(())
    }

    /// Validate prefix coordinates, rejecting inconsistent duplicate claims.
    pub fn from_entries(
        entries: impl IntoIterator<Item = (TimelineId, u64)>,
    ) -> Result<Self, MetadataError> {
        let mut map = BTreeMap::new();
        for (id, n) in entries {
            crate::error::validate_counter(n, "coverage revision")?;
            if map.insert(id, n).is_some_and(|old| old != n) {
                return Err(MetadataError::Conflict {
                    detail: format!("timeline {id} has conflicting prefixes"),
                });
            }
        }
        Ok(Self(map))
    }

    /// Test whether a position is within an explicitly certified prefix.
    pub fn covers(&self, position: Position) -> bool {
        self.0
            .get(&position.timeline)
            .is_some_and(|n| *n >= position.revision)
    }

    /// Iterate exact coordinates for checkpoint persistence and ancestry expansion.
    pub fn entries(&self) -> impl Iterator<Item = (TimelineId, u64)> + '_ {
        self.0.iter().map(|(id, n)| (*id, *n))
    }

    /// Combine independently established complete prefixes.
    pub fn join(&self, other: &Self) -> Self {
        let mut map = self.0.clone();
        for (id, n) in &other.0 {
            map.entry(*id)
                .and_modify(|old| *old = (*old).max(*n))
                .or_insert(*n);
        }
        Self(map)
    }
}

impl TryFrom<Vec<(TimelineId, u64)>> for Coverage {
    type Error = MetadataError;

    fn try_from(value: Vec<(TimelineId, u64)>) -> Result<Self, Self::Error> {
        Self::from_entries(value)
    }
}

impl From<Coverage> for Vec<(TimelineId, u64)> {
    fn from(value: Coverage) -> Self {
        value.0.into_iter().collect()
    }
}

/// Why an ancestry relationship was installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundaryKind {
    /// Initial state of an exceptional restored timeline.
    ForkOrigin,
    /// Complete recovery of an additional older prefix.
    RecoveredCoverage,
}

/// Coverage established at a particular position, never retroactively.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineBoundary {
    /// Earliest receiving position to which this claim applies.
    pub at: Position,
    /// Independently completed older prefixes.
    pub includes: Coverage,
    /// Origin or subsequent recovery.
    pub kind: BoundaryKind,
}

/// Causal relationship between interpretable positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionRelation {
    /// Identical positions.
    Equal,
    /// The left position precedes the right.
    Before,
    /// The right position precedes the left.
    After,
    /// Both histories are known and neither covers the other.
    Divergent,
    /// Required history interpretation is unavailable.
    Unknown(UnknownReason),
}

/// Account-scoped sparse timeline boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "LineageData", into = "LineageData")]
pub struct Lineage {
    /// Owning account.
    account: AccountId,
    /// Initial root history.
    initial: TimelineId,
    /// Immutable installed boundaries.
    boundaries: Vec<TimelineBoundary>,
}

/// Serialization representation validated before becoming lineage.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LineageData {
    /// Owning account.
    account: AccountId,
    /// Initial root history.
    initial: TimelineId,
    /// Position-dependent relationships.
    boundaries: Vec<TimelineBoundary>,
}

impl Lineage {
    /// Initialize the known root history of an account.
    pub fn new(account: AccountId, initial: TimelineId) -> Self {
        Self {
            account,
            initial,
            boundaries: Vec::new(),
        }
    }

    /// Return the account whose positions this evidence interprets.
    pub fn account(&self) -> AccountId {
        self.account
    }

    /// Return the initial root timeline for durable restoration of this ancestry snapshot.
    pub fn initial_timeline(&self) -> TimelineId {
        self.initial
    }

    /// Read retained intervals for durable baseline interpretation.
    pub fn boundaries(&self) -> &[TimelineBoundary] {
        &self.boundaries
    }

    /// Add an immutable relationship, rejecting rewrites, missing origins and cycles.
    pub fn add_boundary(&mut self, boundary: TimelineBoundary) -> Result<(), MetadataError> {
        boundary.at.validate()?;
        boundary.includes.validate()?;
        if let Some(old) = self.boundaries.iter().find(|old| old.at == boundary.at) {
            return if old == &boundary {
                Ok(())
            } else {
                Err(MetadataError::Conflict {
                    detail: format!("boundary {:?} rewritten", boundary.at),
                })
            };
        }
        if boundary.kind == BoundaryKind::ForkOrigin
            && (boundary.at.revision != 0
                || boundary.at.timeline == self.initial
                || self
                    .boundaries
                    .iter()
                    .any(|b| b.at.timeline == boundary.at.timeline))
        {
            return Err(MetadataError::Invalid {
                detail: format!("invalid fork origin {:?}", boundary.at),
            });
        }
        if boundary.kind == BoundaryKind::RecoveredCoverage && !self.known(boundary.at.timeline) {
            return Err(MetadataError::Invalid {
                detail: format!("missing origin for {:?}", boundary.at),
            });
        }
        let mut candidate = self.clone();
        candidate.boundaries.push(boundary);
        for b in &candidate.boundaries {
            candidate.coverage_at(b.at)?;
        }
        *self = candidate;
        Ok(())
    }

    /// Establish complete ancestry at this cut, without borrowing later boundaries.
    pub fn coverage_at(&self, position: Position) -> Result<Coverage, MetadataError> {
        position.validate()?;
        let mut result = Coverage::default();
        let mut stack = vec![(position, false)];
        let mut active = Vec::<Position>::new();
        let mut done = BTreeSet::new();
        while let Some((p, exit)) = stack.pop() {
            if exit {
                active.pop();
                done.insert((p.timeline, p.revision));
                continue;
            }
            if active
                .iter()
                .any(|a| a.timeline == p.timeline && p.revision >= a.revision)
            {
                return Err(MetadataError::Invalid {
                    detail: format!("ancestry cycle at {p:?}"),
                });
            }
            if done.contains(&(p.timeline, p.revision)) {
                continue;
            }
            if !self.known(p.timeline) {
                return Err(MetadataError::Incomplete {
                    detail: format!("missing lineage for {}", p.timeline),
                });
            }
            result = result.join(&Coverage::from_entries([(p.timeline, p.revision)])?);
            active.push(p);
            stack.push((p, true));
            for b in self
                .boundaries
                .iter()
                .filter(|b| b.at.timeline == p.timeline && b.at.revision <= p.revision)
            {
                for (timeline, revision) in b.includes.entries() {
                    stack.push((Position { timeline, revision }, false));
                }
            }
        }
        Ok(result)
    }

    /// Compare positions using their actual cuts and retained ancestry.
    pub fn compare(&self, left: Position, right: Position) -> PositionRelation {
        let (Ok(l), Ok(r)) = (self.coverage_at(left), self.coverage_at(right)) else {
            return PositionRelation::Unknown(UnknownReason::MissingLineage);
        };
        if left == right {
            return PositionRelation::Equal;
        }
        match (l.covers(right), r.covers(left)) {
            (true, false) => PositionRelation::After,
            (false, true) => PositionRelation::Before,
            (false, false) => PositionRelation::Divergent,
            (true, true) => PositionRelation::Unknown(UnknownReason::MissingLineage),
        }
    }

    /// Determine whether a timeline has a retained root interpretation.
    fn known(&self, timeline: TimelineId) -> bool {
        timeline == self.initial
            || self
                .boundaries
                .iter()
                .any(|b| b.at.timeline == timeline && b.kind == BoundaryKind::ForkOrigin)
    }
}

impl TryFrom<LineageData> for Lineage {
    type Error = MetadataError;

    fn try_from(value: LineageData) -> Result<Self, Self::Error> {
        let result = Self {
            account: value.account,
            initial: value.initial,
            boundaries: value.boundaries,
        };
        let mut cuts = BTreeSet::new();
        let mut origins = BTreeSet::new();
        for b in &result.boundaries {
            if !cuts.insert((b.at.timeline, b.at.revision))
                || (b.kind == BoundaryKind::ForkOrigin
                    && (b.at.revision != 0
                        || b.at.timeline == result.initial
                        || !origins.insert(b.at.timeline)))
            {
                return Err(MetadataError::Conflict {
                    detail: format!("duplicate or invalid boundary {:?}", b.at),
                });
            }
            result.coverage_at(b.at)?;
        }
        Ok(result)
    }
}

impl From<Lineage> for LineageData {
    fn from(value: Lineage) -> Self {
        Self {
            account: value.account,
            initial: value.initial,
            boundaries: value.boundaries,
        }
    }
}

impl From<Position> for Coverage {
    fn from(position: Position) -> Self {
        Self([(position.timeline, position.revision)].into())
    }
}
