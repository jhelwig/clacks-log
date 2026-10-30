//! Account-wide downloaded-contact matching and guarded external bindings.

use super::{ExternalBindingGuard, ExternalBindingObservation, operation::ServiceDecision};
use crate::{
    client::VersionCondition,
    record::{AcceptedRecord, Observation, RecordId, RecordState, RecordValue},
};
use clacks_model::service::evidence::processing::{AssociationState, ExternalAssociation};
use clacks_model::{
    contact::{BandFrequency, Qso},
    ids::{AccountId, QsoId},
    recognition::{
        RecognitionCatalog,
        mode::{ModeIdentity, ModeInterpretation},
    },
    service::{
        destination::RemoteRecordId,
        evidence::observation::{ModeReport, RemoteObservation, Reported},
        lotw::{LotwConfiguration, normalize_mode},
    },
};

/// Selected association and the explicit transactional index/target observations.
pub struct BindingInput<'a> {
    /// Account owning both records.
    pub account: AccountId,
    /// Requested binding state.
    pub association: &'a ExternalAssociation,
    /// Unique-index lookup, including certified absence.
    pub index: &'a ExternalBindingObservation,
    /// Selected contact or retained contact deletion.
    pub target: &'a Observation,
}

/// Binding mutation guarded against competing assignments and target changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingChange {
    /// Approved binding values.
    pub association: ExternalAssociation,
    /// Unique namespace condition required in the real transaction.
    pub guard: ExternalBindingGuard,
    /// Selected target version required in the real transaction.
    pub target: VersionCondition,
}

/// Validate a binding choice without treating an omitted index read as absence.
#[tracing::instrument(
    skip_all,
    name = "sync.association.binding.plan",
    level = "debug",
    fields(
        account_id = %input.account,
        qso_id = %input.association.qso,
        association_id = %input.association.id,
        destination_id = %input.association.external.destination,
    ),
)]
pub fn plan_binding(input: &BindingInput<'_>) -> anyhow::Result<ServiceDecision<BindingChange>> {
    let a = input.association;
    anyhow::ensure!(
        input.index.account == input.account
            && input.target.key().account == input.account
            && input.index.external == a.external,
        "binding account/namespace mismatch"
    );
    anyhow::ensure!(
        input.target.key().id == RecordId::Qso(a.qso),
        "binding target differs from selected QSO"
    );
    input.target.validate()?;
    if let Some(b) = &input.index.binding
        && (b.association != a.id
            || b.qso != a.qso
            || (b.state != AssociationState::Active && a.state == AssociationState::Active))
    {
        tracing::debug!(
            event = "association.binding_conflict",
            existing_association_id = %b.association,
            existing_qso_id = %b.qso,
            existing_state = ?b.state,
            requested_state = ?a.state,
        );
        return Ok(ServiceDecision::Review(
            "external record already has a different or historical binding".into(),
        ));
    }
    let valid = matches!(
        (a.state, input.target),
        (
            AssociationState::Active,
            Observation::Accepted(AcceptedRecord {
                state: RecordState::Live(RecordValue::Qso(_)),
                ..
            })
        ) | (
            AssociationState::DeletedQso,
            Observation::Accepted(AcceptedRecord {
                state: RecordState::Deleted,
                ..
            })
        )
    ) || a.state == AssociationState::Superseded;
    if !valid {
        tracing::debug!(event = "association.target_incompatible", requested_state = ?a.state);
        return Ok(ServiceDecision::Incomplete(
            "binding target state unavailable or incompatible".into(),
        ));
    }
    Ok(ServiceDecision::Ready(BindingChange {
        association: a.clone(),
        guard: ExternalBindingGuard {
            account: input.account,
            external: a.external.clone(),
            expected: input.index.binding.clone(),
        },
        target: VersionCondition::from_observation(input.target)?,
    }))
}

/// Read view used for matching a downloaded report across every account tag.
pub struct CandidateInput<'a> {
    /// Reported contact values and source precision.
    pub observation: &'a RemoteObservation,
    /// Account isolation domain.
    pub account: AccountId,
    /// All relevant account contacts, without a selected-tag restriction.
    pub account_qsos: &'a [AcceptedRecord],
    /// Current mode/band interpretation data.
    pub recognition: &'a RecognitionCatalog,
    /// Source-derived LoTW mapping for lossy mode recognition.
    pub lotw_configuration: &'a LotwConfiguration,
}

/// A plausible contact whose missing/differing information requires explicit selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateMatch {
    /// Local contact identity.
    pub qso: QsoId,
    /// Reasons automatic confident association is unsafe.
    pub reasons: Vec<String>,
}

/// Distinct confident and operator-selected matching candidates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CandidateSet {
    /// Contacts agreeing on every confident-match dimension.
    pub confident: Vec<QsoId>,
    /// Compatible contacts requiring explicit selection, even when unique.
    pub possible: Vec<CandidateMatch>,
}

/// Classify actual saved contacts; this never creates a contact or chooses ambiguous matches.
#[tracing::instrument(
    skip_all,
    name = "sync.association.candidates.calculate",
    level = "debug",
    fields(
        account_id = %input.account,
        observation_id = %input.observation.id,
    ),
)]
pub fn classify_candidates(input: &CandidateInput<'_>) -> anyhow::Result<CandidateSet> {
    let mut out = CandidateSet::default();
    for row in input.account_qsos {
        anyhow::ensure!(
            row.key.account == input.account,
            "candidate crosses account"
        );
        row.validate()?;
        if let RecordState::Live(RecordValue::Qso(q)) = &row.state
            && let Some(reasons) = candidate(q, input)?
        {
            if reasons.is_empty() {
                out.confident.push(q.id());
            } else {
                out.possible.push(CandidateMatch {
                    qso: q.id(),
                    reasons,
                });
            }
        }
    }
    out.confident.sort();
    out.confident.dedup();
    out.possible.sort_by_key(|m| m.qso);
    out.possible.dedup_by_key(|m| m.qso);
    Ok(out)
}

/// Compare full identities and precision-aware start intervals for one candidate.
fn candidate(q: &Qso, input: &CandidateInput<'_>) -> anyhow::Result<Option<Vec<String>>> {
    let c = &input.observation.facts.contact;
    let mut reasons = Vec::new();
    for (field, reported, local) in [
        ("own call", &c.own_call, &q.own_station().callsign),
        (
            "worked call",
            &c.worked_call,
            &q.contacted_station().callsign,
        ),
    ] {
        match reported {
            Reported::Value(v) if v != local => return Ok(None),
            Reported::Value(_) => {}
            _ => reasons.push(format!("missing {field}")),
        }
    }
    let receive = q.primary().effective_receive(q.receive_override().as_ref());
    for (field, band, freq, local) in [
        ("band", &c.band, &c.frequency, q.primary()),
        (
            "receive band",
            &c.receive_band,
            &c.receive_frequency,
            receive,
        ),
    ] {
        // Receive absence means no separate override, not missing primary identity.
        if field == "receive band"
            && !matches!(band, Reported::Value(_))
            && !matches!(freq, Reported::Value(_))
        {
            continue;
        }
        let supplied = if let Reported::Value(f) = freq {
            clacks_model::recognition::band::Band::for_frequency(*f, input.recognition)
                .map(|v| v.label().to_owned())
        } else if let Reported::Value(v) = band {
            Some(v.clone())
        } else {
            None
        };
        let native = pair_band(local, input.recognition);
        match (supplied, native) {
            (Some(a), Some(b)) if !a.eq_ignore_ascii_case(&b) => return Ok(None),
            (Some(_), Some(_)) => {}
            _ => reasons.push(format!("incomplete {field}")),
        }
    }
    match &c.mode {
        Reported::Value(m) => match mode_match(q, m, input)? {
            Some(true) => {}
            Some(false) => reasons.push("mode report lacks exact specialization".into()),
            None => return Ok(None),
        },
        _ => reasons.push("missing mode".into()),
    }
    if let Reported::Value(time) = c.start {
        let (lo, hi) = time.bounds()?;
        let t = q.start().unix_seconds();
        let gap = if t < lo {
            i128::from(lo) - i128::from(t)
        } else if t > hi {
            i128::from(t) - i128::from(hi)
        } else {
            0
        };
        if gap > 1800 {
            return Ok(None);
        }
        if gap > 0 {
            reasons.push(format!("start differs by {gap} seconds"));
        }
    } else {
        reasons.push("missing start".into());
    }
    Ok(Some(reasons))
}

/// Resolve a recorded direction without inventing an unknown band.
fn pair_band(pair: &BandFrequency, catalog: &RecognitionCatalog) -> Option<String> {
    pair.frequency()
        .and_then(|f| clacks_model::recognition::band::Band::for_frequency(f, catalog))
        .map(|b| b.label().to_owned())
        .or_else(|| pair.band().map(|b| b.label().to_owned()))
}

/// Return exact agreement, compatible lossy agreement, or explicit contradiction.
fn mode_match(q: &Qso, m: &ModeReport, input: &CandidateInput<'_>) -> anyhow::Result<Option<bool>> {
    let (label, special) = match q.mode().identity() {
        ModeIdentity::Parent { label } => (label, false),
        ModeIdentity::Specialization { label, .. } => (label, true),
    };
    if label.eq_ignore_ascii_case(&m.concrete) {
        return Ok(Some(true));
    }
    if let ModeInterpretation::Known(known) = q.mode().interpret(input.recognition) {
        if special && known.mode.code.eq_ignore_ascii_case(&m.concrete) {
            return Ok(Some(false));
        }
        if !special
            && m.parent
                .as_ref()
                .is_some_and(|p| p.eq_ignore_ascii_case(&known.mode.code))
        {
            return Ok(Some(false));
        }
    }
    if matches!(&input.observation.identity,clacks_model::service::evidence::observation::RemoteIdentity::Complete(k) if matches!(k.record,RemoteRecordId::Lotw(_)))
        && normalize_mode(q.mode(), input.lotw_configuration)
            .is_ok_and(|mapped| mapped.eq_ignore_ascii_case(&m.concrete))
    {
        return Ok(Some(false));
    }
    Ok(None)
}
