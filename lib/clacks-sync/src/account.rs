//! Durable account heads, baselines and complete installed checkpoints.

use crate::{
    error::MetadataError,
    lineage::{Coverage, Lineage},
    record::Position,
};
use clacks_model::ids::{AccountId, BaselineId, TimelineId};
use serde::{Deserialize, Serialize};

/// Interpretable common-history summary replacing eligible old detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// Stable interpretation identity.
    pub id: BaselineId,
    /// Owning account.
    pub account: AccountId,
    /// Certified common prefixes.
    pub coverage: Coverage,
    /// Retained position-dependent ancestry.
    pub lineage: Lineage,
    /// Membership version used to select this baseline.
    pub membership_generation: u64,
}

/// Complete installed state; reload after commit before sending acknowledgment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledCheckpoint {
    /// Owning account.
    pub account: AccountId,
    /// Installed interpretation.
    pub baseline: BaselineId,
    /// Fully represented prefixes.
    pub coverage: Coverage,
}

/// Current server synchronization head and admission epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountMetadata {
    /// Owning account.
    pub account: AccountId,
    /// Current accepted/journal position.
    pub head: Position,
    /// Current baseline interpretation.
    pub baseline: BaselineId,
    /// Complete locally installed coverage.
    pub installed: Coverage,
    /// Current membership version.
    pub membership_generation: u64,
}

impl AccountMetadata {
    /// Initialize empty history; native seed records use ordinary server acceptance.
    pub fn initialize(
        account: AccountId,
        timeline: TimelineId,
        baseline: BaselineId,
    ) -> (Self, Baseline) {
        let coverage = Coverage::from(Position {
            timeline,
            revision: 0,
        });
        (
            Self {
                account,
                head: Position {
                    timeline,
                    revision: 0,
                },
                baseline,
                installed: coverage.clone(),
                membership_generation: 0,
            },
            Baseline {
                id: baseline,
                account,
                coverage,
                lineage: Lineage::new(account, timeline),
                membership_generation: 0,
            },
        )
    }
}

/// Borrowed interpretation from an actual store read view.
pub struct EvidenceView<'a> {
    /// Account-scoped timeline evidence.
    pub lineage: &'a Lineage,
    /// Retained baseline definitions.
    pub baselines: &'a [Baseline],
}

impl EvidenceView<'_> {
    /// Resolve an unambiguous baseline in the requested account.
    pub fn baseline(
        &self,
        account: AccountId,
        id: BaselineId,
    ) -> Result<Option<&Baseline>, MetadataError> {
        if self.lineage.account() != account {
            return Err(MetadataError::Scope {
                detail: format!(
                    "lineage account {} differs from {account}",
                    self.lineage.account()
                ),
            });
        }
        let mut found = None;
        for b in self.baselines.iter().filter(|b| b.id == id) {
            if b.account != account || b.lineage.account() != account {
                return Err(MetadataError::Scope {
                    detail: format!("baseline {id} belongs to another account"),
                });
            }
            if found.is_some_and(|old| old != b) {
                return Err(MetadataError::Conflict {
                    detail: format!("baseline {id} has multiple interpretations"),
                });
            }
            found = Some(b);
        }
        Ok(found)
    }
}

impl AccountMetadata {
    /// Allocate the next mutation/journal position and its complete local prefix.
    ///
    /// Call only as part of a conditional transaction; exhaustion never wraps.
    pub(crate) fn advance(&self) -> Result<Self, MetadataError> {
        let mut next = self.clone();
        next.head.revision =
            next.head
                .revision
                .checked_add(1)
                .ok_or_else(|| MetadataError::Overflow {
                    counter: format!("account {} timeline {}", self.account, self.head.timeline),
                })?;
        next.installed = next.installed.join(&Coverage::from(next.head));
        Ok(next)
    }
}
