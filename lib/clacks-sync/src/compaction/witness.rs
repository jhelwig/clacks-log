//! Restoration and causal interpretation of sealed original evidence.

use super::*;

/// Serialized evidence whose causal meaning must be established on restoration.
#[derive(Deserialize)]
pub(super) struct WitnessFields {
    /// Account-qualified native identity.
    key: RecordKey,
    /// Retained original values and comparison context.
    original: OriginalBasis,
    /// Actual accepted context used to establish ordering.
    observed: VersionStamp,
    /// Complete installed prefixes at observation.
    checkpoint: InstalledCheckpoint,
    /// Established original-to-observed relationship.
    relation: EstablishedBasisRelation,
    /// Timeline ancestry needed to interpret the context.
    lineage: Lineage,
    /// Retained baseline meanings for the supplied contexts.
    baselines: Vec<Baseline>,
}

impl TryFrom<WitnessFields> for BasisWitness {
    type Error = anyhow::Error;

    fn try_from(fields: WitnessFields) -> anyhow::Result<Self> {
        let witness = Self::restore(
            fields.original,
            fields.observed,
            fields.checkpoint,
            fields.lineage,
            fields.baselines,
        )?;
        anyhow::ensure!(
            witness.key == fields.key && witness.relation == fields.relation,
            MetadataError::Conflict {
                detail: "sealed witness differs from retained causal evidence".into()
            }
        );
        Ok(witness)
    }
}

impl BasisWitness {
    /// Reestablish a retained original's ordering using its complete comparison evidence.
    pub fn restore(
        original: OriginalBasis,
        observed: VersionStamp,
        checkpoint: InstalledCheckpoint,
        lineage: Lineage,
        baselines: Vec<Baseline>,
    ) -> anyhow::Result<Self> {
        let key = original.key();
        anyhow::ensure!(
            checkpoint.account == key.account && lineage.account() == key.account,
            MetadataError::Scope {
                detail: "witness evidence crosses account".into()
            }
        );
        for baseline in &baselines {
            baseline.validate()?;
            anyhow::ensure!(
                baseline.account == key.account,
                MetadataError::Scope {
                    detail: "witness baseline crosses account".into()
                }
            );
        }
        let evidence = EvidenceView {
            lineage: &lineage,
            baselines: &baselines,
        };
        anyhow::ensure!(
            expanded(&checkpoint, &evidence)?.covers(observed.accepted_at()),
            MetadataError::Incomplete {
                detail: "witness checkpoint does not cover observation".into()
            }
        );
        let relation = match &original {
            OriginalBasis::Accepted(value) => {
                compare_contexts(key, &value.stamp, &observed, &evidence)?
            }
            OriginalBasis::SubsumedOwn { witness, state, .. } => {
                state.validate(key)?;
                compare_contexts(key, witness, &observed, &evidence)?
            }
            OriginalBasis::NeverCreated {
                checkpoint: source, ..
            } => {
                anyhow::ensure!(
                    source.account == key.account,
                    MetadataError::Scope {
                        detail: "absence basis crosses account".into()
                    }
                );
                anyhow::ensure!(
                    expanded(source, &evidence)?
                        .entries()
                        .all(|(timeline, revision)| checkpoint
                            .coverage
                            .covers(Position { timeline, revision })),
                    MetadataError::Incomplete {
                        detail: "absence basis is not covered by witness checkpoint".into()
                    }
                );
                RecordRelation::Before
            }
            OriginalBasis::Unavailable { .. } => {
                return Err(MetadataError::Incomplete {
                    detail: "cannot seal an unavailable original".into(),
                }
                .into());
            }
        };
        Ok(Self {
            key,
            original,
            observed,
            checkpoint,
            lineage,
            baselines,
            relation: relation.try_into()?,
        })
    }

    /// Return the account-qualified native record described by the evidence.
    pub fn key(&self) -> RecordKey {
        self.key
    }

    /// Borrow the genuine original values and context.
    pub fn original(&self) -> &OriginalBasis {
        &self.original
    }

    /// Borrow the actual observed accepted context.
    pub fn observed(&self) -> &VersionStamp {
        &self.observed
    }

    /// Borrow the complete installed prefix used when sealing.
    pub fn checkpoint(&self) -> &InstalledCheckpoint {
        &self.checkpoint
    }

    /// Return the established original-to-observed ordering.
    pub fn relation(&self) -> EstablishedBasisRelation {
        self.relation
    }

    /// Borrow retained timeline ancestry needed to interpret the witness.
    pub fn lineage(&self) -> &Lineage {
        &self.lineage
    }

    /// Borrow retained baseline meanings needed to interpret the witness.
    pub fn baselines(&self) -> &[Baseline] {
        &self.baselines
    }
}
