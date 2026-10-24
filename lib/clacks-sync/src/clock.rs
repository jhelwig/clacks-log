//! Record-local version vectors; these never certify account-prefix receipt.

use crate::error::MetadataError;
use clacks_model::ids::{ClientActorId, TimelineId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Role-qualified writer of a record version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ClockActor {
    /// An enrolled client preparing a record change.
    Client(ClientActorId),
    /// Server acceptance within one timeline.
    Server(TimelineId),
}

/// Componentwise relationship, meaningful only in a compatible record domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorRelation {
    /// All components agree.
    Equal,
    /// Only the other vector contains later components.
    Before,
    /// Only this vector contains later components.
    After,
    /// Each vector contains a component missing from the other.
    Concurrent,
}

/// Explicit nonzero writer counters for one record and baseline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<(ClockActor, u64)>", into = "Vec<(ClockActor, u64)>")]
pub struct RecordVector(BTreeMap<ClockActor, u64>);

impl RecordVector {
    /// Validate counters, removing zeros and rejecting conflicting duplicates.
    pub fn from_entries(
        entries: impl IntoIterator<Item = (ClockActor, u64)>,
    ) -> Result<Self, MetadataError> {
        let mut all = BTreeMap::new();
        for (actor, counter) in entries {
            crate::error::validate_counter(counter, "record vector component")?;
            if all.insert(actor, counter).is_some_and(|old| old != counter) {
                return Err(MetadataError::Conflict {
                    detail: format!("writer {actor:?} has conflicting counters"),
                });
            }
        }
        all.retain(|_, counter| *counter != 0);
        Ok(Self(all))
    }

    /// Read a residual component; interpret its baseline before comparing records.
    pub fn component(&self, actor: ClockActor) -> u64 {
        self.0.get(&actor).copied().unwrap_or(0)
    }

    /// Compare raw components after the caller establishes compatible baselines.
    pub fn compare_components(&self, other: &Self) -> VectorRelation {
        let mut lower = false;
        let mut higher = false;
        for actor in self.0.keys().chain(other.0.keys()) {
            lower |= self.component(*actor) < other.component(*actor);
            higher |= self.component(*actor) > other.component(*actor);
        }
        match (lower, higher) {
            (false, false) => VectorRelation::Equal,
            (true, false) => VectorRelation::Before,
            (false, true) => VectorRelation::After,
            (true, true) => VectorRelation::Concurrent,
        }
    }

    /// Combine compatible causal contexts without merging native values.
    pub fn join(&self, other: &Self) -> Self {
        let mut joined = self.0.clone();
        for (actor, count) in &other.0 {
            joined
                .entry(*actor)
                .and_modify(|n| *n = (*n).max(*count))
                .or_insert(*count);
        }
        Self(joined)
    }

    /// Reserve a counter above both the observed component and durable high-water mark.
    pub fn increment(
        &self,
        actor: ClockActor,
        high_water: u64,
    ) -> Result<(Self, u64), MetadataError> {
        let count = crate::error::next_counter(
            self.component(actor).max(high_water),
            &format!("{actor:?}"),
        )?;
        let mut next = self.clone();
        next.0.insert(actor, count);
        Ok((next, count))
    }

    /// Iterate retained components for serialization and baseline projection.
    pub fn entries(&self) -> impl Iterator<Item = (ClockActor, u64)> + '_ {
        self.0.iter().map(|(a, n)| (*a, *n))
    }
}

impl TryFrom<Vec<(ClockActor, u64)>> for RecordVector {
    type Error = MetadataError;

    fn try_from(value: Vec<(ClockActor, u64)>) -> Result<Self, Self::Error> {
        Self::from_entries(value)
    }
}

impl From<RecordVector> for Vec<(ClockActor, u64)> {
    fn from(value: RecordVector) -> Self {
        value.0.into_iter().collect()
    }
}
