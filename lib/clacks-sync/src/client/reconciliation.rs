//! Interpretation of retained originals against current shared evidence.

use super::*;

/// Recover private baseline interpretation without extending a witness onto another branch.
pub(super) fn witness_evidence(
    current: &ClientRecord,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Option<(crate::lineage::Lineage, Vec<crate::account::Baseline>)>> {
    let Some(pending) = &current.pending else {
        return Ok(None);
    };
    let Some(witness) = &pending.witness else {
        return Ok(None);
    };
    let mut lineage = evidence.lineage.clone();
    for boundary in witness.lineage.boundaries() {
        lineage.add_boundary(boundary.clone())?;
    }
    let baselines = evidence
        .baselines
        .iter()
        .chain(&witness.baselines)
        .cloned()
        .collect::<Vec<_>>();
    let view = EvidenceView {
        lineage: &lineage,
        baselines: &baselines,
    };
    if !crate::compaction::expanded(&witness.checkpoint, &view)?
        .covers(witness.observed.accepted_at())
    {
        return Err(MetadataError::Incomplete {
            detail: "sealed checkpoint does not cover its observed version".into(),
        }
        .into());
    }
    let original = match &pending.original {
        OriginalBasis::Accepted(v) => Some(&v.stamp),
        OriginalBasis::SubsumedOwn { witness, .. } => Some(witness),
        _ => None,
    };
    if let Some(original) = original {
        let relation =
            crate::compare::compare_contexts(current.key, original, &witness.observed, &view)?;
        if !matches!(
            relation,
            RecordRelation::Before | RecordRelation::SameVersion
        ) {
            return Err(MetadataError::Conflict {
                detail: "sealed relationship has no retained interpretation".into(),
            }
            .into());
        }
    }
    Ok(Some((lineage, baselines)))
}

/// Determine the review reason using whole-record values and retained ordering.
pub(super) fn review_reason(
    pending: &PendingRecord,
    shared: &Observation,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Option<ReviewReason>> {
    let shared_state = match shared {
        Observation::Accepted(v) => Some(&v.state),
        Observation::Absent { .. } => None,
        Observation::Unavailable { reason, .. } => {
            return Ok(Some(ReviewReason::OrderingUnknown(*reason)));
        }
    };
    if shared_state.is_some_and(|s| s.same_values(&pending.working)) {
        return Ok(None);
    }
    if shared_state.is_some_and(|shared| {
        matches!(
            (shared, &pending.working),
            (
                crate::record::RecordState::Live(crate::record::RecordValue::Directory(_)),
                crate::record::RecordState::Live(crate::record::RecordValue::Directory(_))
            )
        )
    }) {
        return Ok(None);
    }
    if let OriginalBasis::Unavailable { .. } = &pending.original {
        return Ok(Some(ReviewReason::MissingOriginal));
    }
    if let (OriginalBasis::Accepted(original), Observation::Accepted(current)) =
        (&pending.original, shared)
        && let RecordRelation::Unknown(reason) = compare_records(original, current, evidence)?
    {
        return Ok(Some(ReviewReason::OrderingUnknown(reason)));
    }
    if let (OriginalBasis::SubsumedOwn { witness, .. }, Observation::Accepted(current)) =
        (&pending.original, shared)
    {
        match crate::compare::compare_contexts(pending.key, witness, &current.stamp, evidence)? {
            RecordRelation::SameVersion | RecordRelation::Before => {}
            RecordRelation::Unknown(reason) => {
                return Ok(Some(ReviewReason::OrderingUnknown(reason)));
            }
            _ => {
                return Ok(Some(ReviewReason::OrderingUnknown(
                    UnknownReason::CollectedContext,
                )));
            }
        }
    }
    let local = Some(&pending.working);
    Ok(
        (classify_record_changes(pending.original.state(), local, shared_state)
            == ChangePattern::BothDifferent)
            .then_some(ReviewReason::BothDifferent),
    )
}
