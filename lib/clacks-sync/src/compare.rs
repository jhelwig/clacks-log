//! Whole-record value policy and baseline-aware causal comparison.

use serde::{Deserialize, Serialize};

use crate::{
    account::EvidenceView,
    clock::{ClockActor, VectorRelation},
    error::{MetadataError, UnknownReason},
    lineage::PositionRelation,
    record::AcceptedRecord,
};

/// Three-way value attribution; use the whole record to decide whether to review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    if left.stamp.accepted_at == right.stamp.accepted_at
        && let (Some(a), Some(b)) = (
            crate::service::payload(left),
            crate::service::payload(right),
        )
    {
        match crate::service::compare_support(a, b, left.key, evidence)? {
            RecordRelation::SameVersion => {}
            RecordRelation::Unknown(reason) => return Ok(RecordRelation::Unknown(reason)),
            _ => {
                return Err(MetadataError::Conflict {
                    detail: format!("different service facts/support at {:?}", left.key),
                }
                .into());
            }
        }
    }
    if left.stamp.accepted_at == right.stamp.accepted_at && !left.state.same_values(&right.state) {
        return Err(MetadataError::Conflict {
            detail: format!(
                "record {:?} has different values at {:?}",
                left.key, left.stamp.accepted_at
            ),
        }
        .into());
    }
    compare_contexts(left.key, &left.stamp, &right.stamp, evidence)
}

/// Interpret context-only descriptors without constructing invented native envelopes.
pub(crate) fn compare_contexts(
    key: crate::record::RecordKey,
    left: &crate::record::VersionStamp,
    right: &crate::record::VersionStamp,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<RecordRelation> {
    let lb = evidence.baseline(key.account, left.baseline)?;
    let rb = evidence.baseline(key.account, right.baseline)?;
    let (Some(lb), Some(rb)) = (lb, rb) else {
        return Ok(RecordRelation::Unknown(UnknownReason::MissingBaseline));
    };
    let positions = evidence
        .lineage
        .compare(left.accepted_at, right.accepted_at);
    let raw = left.vector.compare_components(&right.vector);
    let left_covered = rb.coverage.covers(left.accepted_at);
    let right_covered = lb.coverage.covers(right.accepted_at);
    if left.accepted_at == right.accepted_at {
        let compatible_projection = lb != rb
            && left
                .vector
                .entries()
                .chain(right.vector.entries())
                .all(|(actor, _)| {
                    let (l, r) = (left.vector.component(actor), right.vector.component(actor));
                    l == r || (matches!(actor, ClockActor::Client(_)) && (l == 0 || r == 0))
                });
        if raw == VectorRelation::Equal || compatible_projection {
            return Ok(RecordRelation::SameVersion);
        }
        return Err(MetadataError::Conflict {
            detail: format!(
                "record {:?} has incompatible contexts for the same acceptance",
                key
            ),
        }
        .into());
    }
    // An explicit server component can prove record succession beyond global coverage.
    let l_in_r = right
        .vector
        .component(ClockActor::Server(left.accepted_at.timeline))
        >= left.accepted_at.revision;
    let r_in_l = left
        .vector
        .component(ClockActor::Server(right.accepted_at.timeline))
        >= right.accepted_at.revision;
    let before =
        l_in_r || matches!(positions, PositionRelation::Before) || (left_covered && !right_covered);
    let after =
        r_in_l || matches!(positions, PositionRelation::After) || (right_covered && !left_covered);
    if before && after {
        return Err(MetadataError::Conflict {
            detail: format!("opposite ordering evidence for {:?}", key),
        }
        .into());
    }
    if before {
        if !left_covered
            && left.vector.entries().any(|(actor, n)| {
                matches!(actor, ClockActor::Client(_))
                    && n > right.vector.component(actor)
                    && (lb == rb || right.vector.component(actor) != 0)
            })
        {
            return Err(MetadataError::Conflict {
                detail: format!("successor of {:?} loses client context", key),
            }
            .into());
        }
        return Ok(RecordRelation::Before);
    }
    if after {
        return compare_contexts(key, right, left, evidence).map(|r| match r {
            RecordRelation::Before => RecordRelation::After,
            other => other,
        });
    }
    if lb != rb && !left_covered && !right_covered {
        return Ok(RecordRelation::Unknown(UnknownReason::CollectedContext));
    }
    match raw {
        VectorRelation::Before => Ok(RecordRelation::Before),
        VectorRelation::After => Ok(RecordRelation::After),
        VectorRelation::Equal => Err(MetadataError::Conflict {
            detail: format!("different acceptances of {:?} reuse one context", key),
        }
        .into()),
        VectorRelation::Concurrent => match positions {
            PositionRelation::Unknown(reason) => Ok(RecordRelation::Unknown(reason)),
            _ => Ok(RecordRelation::Divergent),
        },
    }
}

/// Apply whole-record attribution to semantic values rather than support-clock encoding.
pub(crate) fn classify_record_changes(
    original: Option<&crate::record::RecordState>,
    local: Option<&crate::record::RecordState>,
    shared: Option<&crate::record::RecordState>,
) -> ChangePattern {
    let equal =
        |a: Option<&crate::record::RecordState>, b: Option<&crate::record::RecordState>| match (
            a, b,
        ) {
            (Some(a), Some(b)) => a.same_values(b),
            (None, None) => true,
            _ => false,
        };
    match (
        equal(original, local),
        equal(original, shared),
        equal(local, shared),
    ) {
        (true, true, _) => ChangePattern::Unchanged,
        (false, true, _) => ChangePattern::LocalOnly,
        (true, false, _) => ChangePattern::SharedOnly,
        (false, false, true) => ChangePattern::EqualResults,
        (false, false, false) => ChangePattern::BothDifferent,
    }
}
