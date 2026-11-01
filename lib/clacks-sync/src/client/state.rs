//! Native client-state access and checked record/proposal construction.

use super::*;

impl OriginalBasis {
    /// Identify the original's record without inferring a version from current coverage.
    pub fn key(&self) -> RecordKey {
        match self {
            Self::Accepted(v) => v.key,
            Self::NeverCreated { key, .. }
            | Self::SubsumedOwn { key, .. }
            | Self::Unavailable { key, .. } => *key,
        }
    }

    /// Read retained original values; absence and missing originals have no saved state.
    pub fn state(&self) -> Option<&RecordState> {
        match self {
            Self::Accepted(v) => Some(&v.state),
            Self::SubsumedOwn { state, .. } => Some(state),
            _ => None,
        }
    }

    /// Read retained record context for outgoing preparation and compaction witnesses.
    pub fn vector(&self) -> RecordVector {
        match self {
            Self::Accepted(v) => v.stamp.vector().clone(),
            Self::SubsumedOwn {
                vector, witness, ..
            } => vector.join(witness.vector()),
            _ => RecordVector::default(),
        }
    }
}

impl TryFrom<ClientRecordFields> for ClientRecord {
    type Error = anyhow::Error;

    fn try_from(fields: ClientRecordFields) -> anyhow::Result<Self> {
        let value = Self {
            key: fields.key,
            latest: fields.latest,
            retained: fields.retained,
            pending: fields.pending,
        };
        value.validate()?;
        Ok(value)
    }
}

impl ClientRecord {
    /// Return this local record's owning account and native identity.
    pub fn key(&self) -> RecordKey {
        self.key
    }

    /// Borrow the latest incorporated shared observation.
    pub fn latest(&self) -> &Observation {
        &self.latest
    }

    /// Borrow accepted alternatives retained for reconciliation.
    pub fn retained(&self) -> &[AcceptedRecord] {
        &self.retained
    }

    /// Borrow unpublished work and its frozen proposal, when present.
    pub fn pending(&self) -> Option<&PendingRecord> {
        self.pending.as_ref()
    }

    /// Recover input for a complete checked replacement of the local state.
    pub fn into_fields(self) -> ClientRecordFields {
        ClientRecordFields {
            key: self.key,
            latest: self.latest,
            retained: self.retained,
            pending: self.pending,
        }
    }

    /// Borrow saved local working values, using incorporated values when no unpublished work exists.
    ///
    /// Pending deletions remain `Some(RecordState::Deleted)`. `None` means that
    /// this row has no saved native value; inspect the shared observation for its evidence.
    pub fn working_state(&self) -> Option<&RecordState> {
        match &self.pending {
            Some(pending) => Some(&pending.working),
            None => match &self.latest {
                Observation::Accepted(value) => Some(&value.state),
                Observation::Absent { .. } | Observation::Unavailable { .. } => None,
            },
        }
    }

    /// Check native values, record scope and retained comparison structure before persistence or reconciliation.
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        self.latest.validate()?;
        if self.latest.key() != self.key {
            return Err(MetadataError::Scope {
                detail: format!("client latest key differs from {:?}", self.key),
            }
            .into());
        }
        for retained in &self.retained {
            retained.validate()?;
            if retained.key != self.key {
                return Err(MetadataError::Scope {
                    detail: format!("retained accepted context differs from {:?}", self.key),
                }
                .into());
            }
        }
        if let Some(p) = &self.pending {
            if p.key != self.key || p.original.key() != self.key {
                return Err(MetadataError::Scope {
                    detail: format!("pending/original key differs from {:?}", self.key),
                }
                .into());
            }
            p.working.validate(self.key)?;
            match &p.original {
                OriginalBasis::Accepted(original) => original.validate()?,
                OriginalBasis::NeverCreated { checkpoint, .. } => {
                    if let Some(checkpoint) = checkpoint {
                        checkpoint.validate()?;
                        anyhow::ensure!(
                            checkpoint.account == self.key.account,
                            "absence basis crosses account"
                        );
                    }
                }
                OriginalBasis::SubsumedOwn { .. } | OriginalBasis::Unavailable { .. } => {}
            }
            if let Some(state) = p.original.state() {
                state.validate(self.key)?;
            }
            if let Some(prepared) = &p.prepared {
                if prepared.key != p.key || prepared.expected.key() != p.key {
                    return Err(MetadataError::Scope {
                        detail: format!("prepared key differs from {:?}", p.key),
                    }
                    .into());
                }
                prepared.state.validate(p.key)?;
                prepared.expected.validate()?;
                for dependency in &prepared.dependencies {
                    dependency.validate()?;
                }
            }
            if let Some(witness) = &p.witness {
                witness.checkpoint.validate()?;
                anyhow::ensure!(
                    witness.key == self.key
                        && witness.original == p.original
                        && witness.checkpoint.account == self.key.account
                        && witness.lineage.account() == self.key.account
                        && matches!(
                            witness.relation,
                            crate::compaction::EstablishedBasisRelation::Before
                                | crate::compaction::EstablishedBasisRelation::SameVersion
                        ),
                    MetadataError::Conflict {
                        detail: format!(
                            "sealed witness does not describe pending original {:?}",
                            self.key
                        )
                    }
                );
                for baseline in &witness.baselines {
                    baseline.validate()?;
                    anyhow::ensure!(
                        baseline.account == self.key.account,
                        MetadataError::Scope {
                            detail: "witness baseline crosses account".into()
                        }
                    );
                }
            }
        }
        Ok(())
    }
}

impl TryFrom<PreparedChangeFields> for PreparedChange {
    type Error = anyhow::Error;

    fn try_from(fields: PreparedChangeFields) -> anyhow::Result<Self> {
        fields.state.validate(fields.key)?;
        fields.expected.validate()?;
        anyhow::ensure!(
            fields.expected.key() == fields.key,
            MetadataError::Scope {
                detail: "prepared condition differs from proposal identity".into()
            }
        );
        anyhow::ensure!(
            fields.vector.component(ClockActor::Client(fields.actor)) > 0,
            MetadataError::Invalid {
                detail: format!("proposal {:?} has no client contribution", fields.key)
            }
        );
        for dependency in &fields.dependencies {
            dependency.validate()?;
        }
        Ok(Self {
            key: fields.key,
            state: fields.state,
            vector: fields.vector,
            actor: fields.actor,
            expected: fields.expected,
            dependencies: fields.dependencies,
        })
    }
}

impl PreparedChange {
    /// Return the owning account and native identity of the proposal.
    pub fn key(&self) -> RecordKey {
        self.key
    }

    /// Borrow the exact native state frozen for transmission.
    pub fn state(&self) -> &RecordState {
        &self.state
    }

    /// Borrow the original context plus the reserved client counter.
    pub fn vector(&self) -> &RecordVector {
        &self.vector
    }

    /// Return the client actor submitting this proposal.
    pub fn actor(&self) -> ClientActorId {
        self.actor
    }

    /// Borrow the exact target version approved for this change.
    pub fn expected(&self) -> &VersionCondition {
        &self.expected
    }

    /// Borrow the compared native reference conditions.
    pub fn dependencies(&self) -> &[VersionCondition] {
        &self.dependencies
    }
}
