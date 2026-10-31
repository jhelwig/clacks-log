//! Retained comparison witnesses and interpretation of compacted coverage.

mod witness;

use crate::{
    account::{Baseline, EvidenceView, InstalledCheckpoint},
    client::{OriginalBasis, PendingRecord},
    compare::{RecordRelation, compare_contexts, compare_records},
    error::MetadataError,
    lineage::{Coverage, Lineage},
    record::{AcceptedRecord, Position, RecordKey, VersionStamp},
};
use serde::{Deserialize, Serialize};

/// Self-contained retained original values and their established comparison relationship.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "witness::WitnessFields")]
pub struct BasisWitness {
    /// Native identity to which this relationship applies.
    pub(crate) key: RecordKey,
    /// Actual original values and context, never relabeled to the checkpoint.
    pub(crate) original: OriginalBasis,
    /// Actual observed accepted version.
    pub(crate) observed: VersionStamp,
    /// Complete installed prefixes at observation.
    pub(crate) checkpoint: InstalledCheckpoint,
    /// Established original-to-observed relationship.
    pub(crate) relation: EstablishedBasisRelation,
    /// Position-dependent ancestry needed to interpret the witness.
    pub(crate) lineage: Lineage,
    /// Baseline meanings needed by the original, observed version and checkpoint.
    pub(crate) baselines: Vec<Baseline>,
}

/// Expand certified checkpoint coordinates using retained ancestry and its baseline.
pub(crate) fn expanded(
    checkpoint: &InstalledCheckpoint,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Coverage> {
    checkpoint.validate()?;
    let baseline = evidence
        .baseline(checkpoint.account, checkpoint.baseline)?
        .ok_or_else(|| MetadataError::Incomplete {
            detail: format!("checkpoint baseline {} unavailable", checkpoint.baseline),
        })?;
    let mut result = checkpoint.coverage.join(&baseline.coverage);
    for (timeline, revision) in checkpoint.coverage.entries() {
        result = result.join(
            &evidence
                .lineage
                .coverage_at(Position { timeline, revision })?,
        );
    }
    Ok(result)
}

/// Retain original values and a proven relationship before releasing older detail.
///
/// Missing or divergent evidence rejects sealing; preserve the old context instead.
#[tracing::instrument(
    skip_all,
    name = "sync.compaction.basis.plan",
    level = "debug",
    fields(
        account_id = %pending.key.account,
        record_id = %pending.key.id,
    ),
)]
pub fn seal_basis(
    pending: &PendingRecord,
    observed: &AcceptedRecord,
    checkpoint: &InstalledCheckpoint,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<BasisWitness> {
    observed.validate()?;
    pending.working.validate(pending.key)?;
    if pending.key != observed.key
        || pending.original.key() != pending.key
        || checkpoint.account != pending.key.account
    {
        return Err(MetadataError::Scope {
            detail: format!("basis witness domain differs from {:?}", pending.key),
        }
        .into());
    }
    if !expanded(checkpoint, evidence)?.covers(observed.stamp.accepted_at()) {
        return Err(MetadataError::Incomplete {
            detail: format!(
                "witness checkpoint does not cover observed {:?}",
                observed.stamp.accepted_at()
            ),
        }
        .into());
    }
    let relation = match &pending.original {
        OriginalBasis::Accepted(original) => compare_records(original, observed, evidence)?,
        OriginalBasis::SubsumedOwn { witness, .. } => {
            compare_contexts(pending.key, witness, &observed.stamp, evidence)?
        }
        OriginalBasis::NeverCreated {
            checkpoint: original,
            ..
        } => {
            let original = original.as_ref().ok_or_else(|| MetadataError::Incomplete {
                detail: "local creation has no certified absence checkpoint".into(),
            })?;
            let source = expanded(original, evidence)?;
            if source.entries().all(|(timeline, revision)| {
                checkpoint.coverage.covers(Position { timeline, revision })
            }) {
                RecordRelation::Before
            } else {
                return Err(MetadataError::Incomplete {
                    detail: "absence basis is not covered by witness checkpoint".into(),
                }
                .into());
            }
        }
        OriginalBasis::Unavailable { .. } => {
            return Err(MetadataError::Incomplete {
                detail: "cannot seal an unavailable original".into(),
            }
            .into());
        }
    };
    if !matches!(
        relation,
        RecordRelation::SameVersion | RecordRelation::Before
    ) {
        return Err(MetadataError::Incomplete {
            detail: format!("cannot seal original relationship {relation:?}"),
        }
        .into());
    }
    let relation = EstablishedBasisRelation::try_from(relation)?;
    let original_baseline = match &pending.original {
        OriginalBasis::Accepted(value) => Some(value.stamp.baseline()),
        OriginalBasis::SubsumedOwn { witness, .. } => Some(witness.baseline()),
        OriginalBasis::NeverCreated { checkpoint, .. } => {
            checkpoint.as_ref().map(|value| value.baseline)
        }
        OriginalBasis::Unavailable { .. } => None,
    };
    let baselines = evidence
        .baselines
        .iter()
        .filter(|baseline| {
            baseline.id == checkpoint.baseline
                || baseline.id == observed.stamp.baseline()
                || Some(baseline.id) == original_baseline
        })
        .cloned()
        .collect();
    Ok(BasisWitness {
        key: pending.key,
        original: pending.original.clone(),
        observed: observed.stamp.clone(),
        checkpoint: checkpoint.clone(),
        relation,
        lineage: evidence.lineage.clone(),
        baselines,
    })
}

/// Proven original-to-observed ordering retained by a sealed basis witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EstablishedBasisRelation {
    /// Original and observation describe the same acceptance.
    SameVersion,
    /// Original causally precedes the observation.
    Before,
}

impl TryFrom<RecordRelation> for EstablishedBasisRelation {
    type Error = MetadataError;

    fn try_from(value: RecordRelation) -> Result<Self, Self::Error> {
        match value {
            RecordRelation::SameVersion => Ok(Self::SameVersion),
            RecordRelation::Before => Ok(Self::Before),
            other => Err(MetadataError::Incomplete {
                detail: format!("cannot seal original relationship {other:?}"),
            }),
        }
    }
}
