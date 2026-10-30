//! Preparing coalesced client proposals and explicit review resolutions.

use super::*;

/// Reserve one coalesced outgoing version or return preserved retry/review state.
///
/// Save a ready counter and row set atomically before sending. The supplied counter
/// belongs to this record and actor; counter exhaustion returns a concrete error.
#[tracing::instrument(
    skip_all,
    name = "sync.client.prepare",
    level = "debug",
    fields(
        account_id = %current.key.account,
        record_id = %current.key.id,
        actor_id = %actor,
    ),
)]
pub fn prepare_change(
    current: &ClientRecord,
    actor: ClientActorId,
    counter: &LocalCounter,
    dependencies: &[VersionCondition],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Preparation> {
    crate::error::validate_counter(counter.high_water, "client counter high-water mark")?;
    for dependency in dependencies {
        dependency.validate()?;
    }
    current.validate()?;
    let sealed = witness_evidence(current, evidence)?;
    let sealed_view = sealed
        .as_ref()
        .map(|(lineage, baselines)| EvidenceView { lineage, baselines });
    let evidence = sealed_view.as_ref().unwrap_or(evidence);
    if counter.actor != actor {
        return Err(MetadataError::Scope {
            detail: format!("counter actor {} differs from {actor}", counter.actor),
        }
        .into());
    }
    let mut record = current.clone();
    if record.pending.is_none()
        && !record.retained.is_empty()
        && let Observation::Accepted(shared) = &record.latest
    {
        record.pending = Some(PendingRecord {
            key: record.key,
            original: OriginalBasis::Accepted(shared.clone()),
            working: shared.state.clone(),
            prepared: None,
            witness: None,
        });
    }
    let Some(p) = &record.pending else {
        return Ok(Preparation::Unchanged { record });
    };
    if let Some(change) = &p.prepared {
        if change.actor != actor {
            tracing::debug!(
                event = "client.review_required",
                account_id = %current.key.account,
                record_id = %current.key.id,
                review_reason = ?ReviewReason::ExcludedActor(change.actor),
            );
            return Ok(Preparation::Review {
                record: record.clone(),
                input: ReviewInput {
                    original: p.original.clone(),
                    local: p.working.clone(),
                    shared: record.latest.clone(),
                    reason: ReviewReason::ExcludedActor(change.actor),
                    retained: record.retained.clone(),
                },
            });
        }
        tracing::debug!(
            event = "client.retry_prepared",
            account_id = %current.key.account,
            record_id = %current.key.id,
        );
        return Ok(Preparation::Retry {
            record: record.clone(),
            change: change.clone(),
        });
    }
    if let Some(reason) = review_reason(p, &record.latest, evidence)? {
        tracing::debug!(
            event = "client.review_required",
            account_id = %current.key.account,
            record_id = %current.key.id,
            review_reason = ?reason,
        );
        return Ok(Preparation::Review {
            record: record.clone(),
            input: ReviewInput {
                original: p.original.clone(),
                local: p.working.clone(),
                shared: record.latest.clone(),
                reason,
                retained: record.retained.clone(),
            },
        });
    }
    let shared = match &record.latest {
        Observation::Accepted(v) => Some(&v.state),
        _ => None,
    };
    let pattern = classify_record_changes(p.original.state(), Some(&p.working), shared);
    if record.retained.is_empty()
        && matches!(
            pattern,
            ChangePattern::Unchanged | ChangePattern::SharedOnly | ChangePattern::EqualResults
        )
    {
        record.pending = None;
        return Ok(Preparation::Unchanged { record });
    }
    let mut context = p.original.vector();
    for accepted in record.retained.iter().chain(match &record.latest {
        Observation::Accepted(v) => Some(v),
        _ => None,
    }) {
        if evidence
            .baseline(record.key.account, accepted.stamp.baseline())?
            .is_none()
        {
            return Err(MetadataError::Incomplete {
                detail: format!(
                    "context reconciliation needs baseline {}",
                    accepted.stamp.baseline()
                ),
            }
            .into());
        }
        context = context.join(accepted.stamp.vector());
    }
    let (vector, high_water) = context.increment(ClockActor::Client(actor), counter.high_water)?;
    for dep in dependencies {
        if dep.key().account != record.key.account {
            return Err(MetadataError::Scope {
                detail: format!("dependency {:?} crosses account", dep.key()),
            }
            .into());
        }
    }
    let change = PreparedChange {
        key: record.key,
        state: p.working.clone(),
        vector,
        actor,
        expected: VersionCondition::from_observation(&record.latest)?,
        dependencies: dependencies.to_vec(),
    };
    if let Some(p) = &mut record.pending {
        p.prepared = Some(change.clone());
    }
    Ok(Preparation::Ready {
        record,
        counter: LocalCounter { actor, high_water },
        change,
    })
}

/// Validate an explicit user-selected result against the compared shared version.
///
/// This creates local pending values; it does not publish or silently restore references.
#[tracing::instrument(
    skip_all,
    name = "sync.client.resolve",
    level = "info",
    fields(
        review_reason = ?review.reason,
    ),
)]
pub fn resolve_review(
    review: &ReviewInput,
    chosen: RecordState,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<ClientRecord> {
    let key = review.shared.key();
    review.shared.validate()?;
    chosen.validate(key)?;
    if key != review.original.key() {
        return Err(MetadataError::Scope {
            detail: format!("review original differs from {key:?}"),
        }
        .into());
    }
    let original = match &review.shared {
        Observation::Accepted(v) => {
            evidence.baseline(key.account, v.stamp.baseline())?;
            OriginalBasis::Accepted(v.clone())
        }
        Observation::Absent { key, checkpoint } => OriginalBasis::NeverCreated {
            key: *key,
            checkpoint: checkpoint.clone(),
        },
        Observation::Unavailable { .. } => {
            return Err(MetadataError::Incomplete {
                detail: format!("cannot resolve against unavailable shared state for {key:?}"),
            }
            .into());
        }
    };
    tracing::info!(
        event = "client.resolution_prepared",
        account_id = %key.account,
        record_id = %key.id,
        review_reason = ?review.reason,
    );
    Ok(ClientRecord {
        retained: review.retained.clone(),
        key,
        latest: review.shared.clone(),
        pending: Some(PendingRecord {
            witness: None,
            key,
            original,
            working: chosen,
            prepared: None,
        }),
    })
}
