//! Exact durable comparison inputs and native source selection.

use clacks_sync::{
    client::{ClientRecord, ReviewInput},
    record::{Observation, RecordState, VersionStamp},
};

/// The exact record and comparison shown to the operator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComparisonBasis {
    /// Saved client roles guarded at commit.
    pub record: ClientRecord,
    /// Persisted local/original/shared and retained comparison evidence.
    pub review: ReviewInput,
}

/// Source for an explicit copy, not a last-writer-wins rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ComparisonSource {
    /// Current saved local working values.
    Local,
    /// Actual compared shared values.
    Shared,
    /// Another retained accepted version for this identity.
    Retained {
        /// Native accepted context, independent of list position.
        stamp: VersionStamp,
    },
}

impl ComparisonBasis {
    /// Borrow an actual source value, refusing unavailable evidence or stale sources.
    pub fn source(&self, source: &ComparisonSource) -> anyhow::Result<&RecordState> {
        match source {
            ComparisonSource::Local => Ok(&self.review.local),
            ComparisonSource::Shared => match &self.review.shared {
                Observation::Accepted(value) => Ok(value.state()),
                _ => anyhow::bail!("Shared values are not available for this comparison"),
            },
            ComparisonSource::Retained { stamp } => self
                .review
                .retained
                .iter()
                .find(|value| &value.stamp == stamp)
                .map(|value| value.state())
                .ok_or_else(|| anyhow::anyhow!("This retained comparison is no longer available")),
        }
    }
}
