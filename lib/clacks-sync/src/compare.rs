//! Whole-record value policy and baseline-aware causal comparison.

use crate::{
    account::EvidenceView,
    clock::{ClockActor, VectorRelation},
    error::{MetadataError, UnknownReason},
    lineage::PositionRelation,
    record::AcceptedRecord,
};

/// Three-way value attribution; use the whole record to decide whether to review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePattern {
    /// Neither side changed.
    Unchanged,
    /// Only local values changed.
    LocalOnly,
    /// Only shared values changed.
    SharedOnly,
    /// Both sides reached the same value.
    EqualResults,
    /// Both sides changed to differing values.
    BothDifferent,
}

/// Classify values without merging fields or interpreting causal metadata.
pub fn classify_changes<T: Eq>(original: &T, local: &T, shared: &T) -> ChangePattern {
    match (original == local, original == shared, local == shared) {
        (true, true, _) => ChangePattern::Unchanged,
        (false, true, _) => ChangePattern::LocalOnly,
        (true, false, _) => ChangePattern::SharedOnly,
        (false, false, true) => ChangePattern::EqualResults,
        (false, false, false) => ChangePattern::BothDifferent,
    }
}

/// Established ordering of two accepted versions of one record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordRelation {
    /// Same logical version, including a compatible projected representation.
    SameVersion,
    /// The left version is represented by the right.
    Before,
    /// The right version is represented by the left.
    After,
    /// Complete compatible evidence establishes different branches.
    Divergent,
    /// Required baseline or ancestry interpretation is unavailable.
    Unknown(UnknownReason),
}

/// Compare validated accepted envelopes without turning record knowledge into coverage.
///
/// Contradictory identities, payloads or ordering evidence return concrete errors.
/// Missing interpretation is an explained unknown result, not concurrency.
pub fn compare_records(
    left: &AcceptedRecord,
    right: &AcceptedRecord,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<RecordRelation> {
    left.validate()?;
    right.validate()?;
    if left.key != right.key {
        return Err(MetadataError::Scope {
            detail: format!("comparison {:?} versus {:?}", left.key, right.key),
        }
        .into());
    }
    let lb = evidence.baseline(left.key.account, left.stamp.baseline)?;
    let rb = evidence.baseline(right.key.account, right.stamp.baseline)?;
    if left.stamp.accepted_at == right.stamp.accepted_at && left.state != right.state {
        return Err(MetadataError::Conflict {
            detail: format!(
                "record {:?} has different values at {:?}",
                left.key, left.stamp.accepted_at
            ),
        }
        .into());
    }
    let (Some(lb), Some(rb)) = (lb, rb) else {
        return Ok(RecordRelation::Unknown(UnknownReason::MissingBaseline));
    };
    let positions = evidence
        .lineage
        .compare(left.stamp.accepted_at, right.stamp.accepted_at);
    let raw = left.stamp.vector.compare_components(&right.stamp.vector);
    let left_covered = rb.coverage.covers(left.stamp.accepted_at);
    let right_covered = lb.coverage.covers(right.stamp.accepted_at);
    if left.stamp.accepted_at == right.stamp.accepted_at {
        if raw == VectorRelation::Equal || (lb != rb && (left_covered || right_covered)) {
            return Ok(RecordRelation::SameVersion);
        }
        return Err(MetadataError::Conflict {
            detail: format!(
                "record {:?} has incompatible contexts for the same acceptance",
                left.key
            ),
        }
        .into());
    }
    if lb != rb && !left_covered && !right_covered {
        return Ok(RecordRelation::Unknown(UnknownReason::CollectedContext));
    }
    // An explicit server component can prove record succession beyond global coverage.
    let l_in_r = right
        .stamp
        .vector
        .component(ClockActor::Server(left.stamp.accepted_at.timeline))
        >= left.stamp.accepted_at.revision;
    let r_in_l = left
        .stamp
        .vector
        .component(ClockActor::Server(right.stamp.accepted_at.timeline))
        >= right.stamp.accepted_at.revision;
    let before =
        l_in_r || matches!(positions, PositionRelation::Before) || (left_covered && !right_covered);
    let after =
        r_in_l || matches!(positions, PositionRelation::After) || (right_covered && !left_covered);
    if before && after {
        return Err(MetadataError::Conflict {
            detail: format!("opposite ordering evidence for {:?}", left.key),
        }
        .into());
    }
    if before {
        if !left_covered
            && left.stamp.vector.entries().any(|(actor, n)| {
                matches!(actor, ClockActor::Client(_)) && n > right.stamp.vector.component(actor)
            })
        {
            return Err(MetadataError::Conflict {
                detail: format!("successor of {:?} loses client context", left.key),
            }
            .into());
        }
        return Ok(RecordRelation::Before);
    }
    if after {
        return compare_records(right, left, evidence).map(|r| match r {
            RecordRelation::Before => RecordRelation::After,
            other => other,
        });
    }
    match raw {
        VectorRelation::Before => Ok(RecordRelation::Before),
        VectorRelation::After => Ok(RecordRelation::After),
        VectorRelation::Equal => Err(MetadataError::Conflict {
            detail: format!("different acceptances of {:?} reuse one context", left.key),
        }
        .into()),
        VectorRelation::Concurrent => match positions {
            PositionRelation::Unknown(reason) => Ok(RecordRelation::Unknown(reason)),
            _ => Ok(RecordRelation::Divergent),
        },
    }
}
