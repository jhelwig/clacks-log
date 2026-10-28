//! Immutable evidence combination and field-scoped provider state selection.

use super::{
    ServiceSupport, ServiceView, check_integrity,
    dependency::{DependencyStatus, SyncServiceError, check},
    payload,
};
use crate::{
    compare::{RecordRelation, compare_records},
    lineage::PositionRelation,
    record::AcceptedRecord,
};
use clacks_model::{
    ids::ObservationId,
    service::{
        destination::ExternalRecordKey,
        evidence::{
            ServicePayload,
            observation::{
                EstablishedObservation, ProviderOrdering, RemoteFacts, RemoteIdentity,
                RemoteObservation, Reported, StationReport,
            },
        },
    },
};

/// Evidence-specific result; provider facts do not enter manual QSO conflict review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceCombination {
    /// The immutable value and logical context are already represented.
    Identical,
    /// Equal content has distinct context requiring the existing context-join path.
    ContextJoin,
    /// Independently identified facts remain separate.
    KeepBoth,
    /// Incoming maintained state is a justified successor.
    ReplaceMaintained,
    /// Incompatible maintained bindings need existing client resolution.
    ReviewBinding,
    /// Required support or ordering interpretation is absent.
    Incomplete,
}

/// Combine retained accepted service records without inventing fresh observations or dispatches.
pub fn combine(
    existing: &AcceptedRecord,
    incoming: &AcceptedRecord,
    view: &ServiceView<'_>,
) -> anyhow::Result<ServiceCombination> {
    existing.validate()?;
    incoming.validate()?;
    if existing.key.account != view.account || incoming.key.account != view.account {
        return Err(SyncServiceError::Scope {
            account: view.account,
            detail: "combination crosses account".into(),
        }
        .into());
    }
    if existing.key != incoming.key {
        return Ok(ServiceCombination::KeepBoth);
    }
    check_integrity(existing.key, &existing.state, &incoming.state)?;
    let Some(record) = payload(incoming) else {
        return Ok(ServiceCombination::ReviewBinding);
    };
    if matches!(check(record, view)?, DependencyStatus::Incomplete(_)) {
        return Ok(ServiceCombination::Incomplete);
    }
    let relation = compare_records(existing, incoming, view.evidence)?;
    if record.payload.immutable() {
        return Ok(
            if matches!(
                relation,
                RecordRelation::SameVersion | RecordRelation::After
            ) {
                ServiceCombination::Identical
            } else {
                ServiceCombination::ContextJoin
            },
        );
    }
    Ok(match relation {
        RecordRelation::SameVersion | RecordRelation::After => ServiceCombination::Identical,
        RecordRelation::Before => ServiceCombination::ReplaceMaintained,
        RecordRelation::Divergent | RecordRelation::Unknown(_) => ServiceCombination::ReviewBinding,
    })
}

/// Inputs for current provider state selection across retained observation occurrences.
pub struct ObservationSelectionInput<'a> {
    /// Exact external record being evaluated.
    pub external: &'a ExternalRecordKey,
    /// Account-local retained evidence and interpretation.
    pub view: &'a ServiceView<'a>,
}

/// Selected information plus explicit unknown paths and retained provenance IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedObservations {
    /// Established per-field state; unordered differing paths supply no value.
    pub current: EstablishedObservation,
    /// Retained observations supporting this calculation.
    pub supporting: Vec<ObservationId>,
    /// Observations whose supplied fields are all undominated and established.
    pub current_supporting: Vec<ObservationId>,
    /// Observations establishing the selected confirmation field specifically.
    ///
    /// Use these to locate the confirmed QSO baseline even when independently
    /// ordered receipt fields have newer supporting observations.
    pub confirmation_supporting: Vec<ObservationId>,
    /// Paths for which current ordering/value cannot be established.
    pub uncertain: Vec<String>,
}

/// Compare properly scoped provider updates, then original acquisition context, never response arrival.
fn order(
    a: &RemoteObservation,
    sa: &ServiceSupport,
    b: &RemoteObservation,
    sb: &ServiceSupport,
    confirmation: bool,
    view: &ServiceView<'_>,
) -> anyhow::Result<Option<std::cmp::Ordering>> {
    let pair = if confirmation {
        a.ordering.confirmation.zip(b.ordering.confirmation)
    } else {
        a.ordering.qso.zip(b.ordering.qso)
    };
    if let Some((a, b)) = pair
        && a != b
    {
        return Ok(Some(a.cmp(&b)));
    }
    let (Some(a), Some(b)) = (&sa.acquisition, &sb.acquisition) else {
        return Ok(None);
    };
    if view.evidence.baseline(view.account, a.baseline)?.is_none()
        || view.evidence.baseline(view.account, b.baseline)?.is_none()
    {
        return Ok(None);
    }
    Ok(
        match view.evidence.lineage.compare(a.accepted_at, b.accepted_at) {
            PositionRelation::Before => Some(std::cmp::Ordering::Less),
            PositionRelation::After => Some(std::cmp::Ordering::Greater),
            PositionRelation::Equal => Some(std::cmp::Ordering::Equal),
            _ => None,
        },
    )
}

/// Select undominated supplied values for a field; contradictory maxima stay explicitly unknown.
fn select<T: Clone + Eq>(
    values: Vec<(&RemoteObservation, &ServiceSupport, &Reported<T>)>,
    confirmation: bool,
    path: &str,
    view: &ServiceView<'_>,
    uncertain: &mut Vec<String>,
    supporting: &mut std::collections::BTreeSet<ObservationId>,
    superseded: &mut std::collections::BTreeSet<ObservationId>,
) -> anyhow::Result<Reported<T>> {
    let values = values
        .into_iter()
        .filter(|(_, _, v)| !matches!(v, Reported::NotReported))
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    for (i, (a, sa, _)) in values.iter().enumerate() {
        for (j, (b, sb, _)) in values.iter().enumerate() {
            if i != j && order(a, sa, b, sb, confirmation, view)? == Some(std::cmp::Ordering::Less)
            {
                edges.push((i, j));
            }
        }
    }
    // Conflicting order can form a cycle. Keep every value in each maximal
    // mutually reachable component instead of silently discarding that component.
    let mut reachability = Vec::new();
    for (i, _) in values.iter().enumerate() {
        let mut reached = std::collections::BTreeSet::from([i]);
        let mut pending = vec![i];
        while let Some(node) = pending.pop() {
            for (_, target) in edges.iter().filter(|(source, _)| *source == node) {
                if reached.insert(*target) {
                    pending.push(*target);
                }
            }
        }
        reachability.push((i, reached));
    }
    let mut maximal = Vec::new();
    for ((i, reached), (observation, _, value)) in reachability.iter().zip(&values) {
        let covered = reached.iter().any(|j| {
            reachability
                .iter()
                .find(|(node, _)| node == j)
                .is_some_and(|(_, other)| !other.contains(i))
        });
        if covered {
            superseded.insert(observation.id);
        } else {
            maximal.push((observation.id, *value));
        }
    }
    let Some(first) = maximal.first() else {
        return Ok(Reported::NotReported);
    };
    if maximal.iter().all(|(_, value)| value == &first.1) {
        supporting.extend(maximal.iter().map(|(id, _)| *id));
        Ok(first.1.clone())
    } else {
        tracing::debug!(
            event = "observation.field_uncertain",
            field = path,
            conflicting_observation_count = maximal.len(),
        );
        uncertain.push(path.into());
        Ok(Reported::NotReported)
    }
}

/// Derive current fields without collapsing independent provider timestamp scopes or hiding uncertainty.
#[tracing::instrument(
    skip_all,
    name = "sync.observations.select",
    level = "debug",
    fields(
        account_id = %input.view.account,
        destination_id = %input.external.destination,
    ),
)]
pub fn select_observations(
    input: &ObservationSelectionInput<'_>,
) -> anyhow::Result<SelectedObservations> {
    let view = input.view;
    let mut reports = Vec::new();
    let mut current_supporting = std::collections::BTreeSet::new();
    let mut superseded = std::collections::BTreeSet::new();
    let mut uncertain = Vec::new();
    for row in view.records {
        if row.key.account != view.account {
            return Err(SyncServiceError::Scope {
                account: view.account,
                detail: "observation selection crosses account".into(),
            }
            .into());
        }
        if let Some(carrier) = payload(row)
            && let ServicePayload::Observation(o) = &carrier.payload
            && matches!(&o.identity,RemoteIdentity::Complete(k) if k==input.external)
        {
            row.validate()?;
            if let DependencyStatus::Incomplete(missing) = check(carrier, view)? {
                uncertain.push(format!("observation {} missing support: {missing:?}", o.id));
                continue;
            }
            reports.push((o.as_ref(), &carrier.support));
        }
    }
    let mut facts = RemoteFacts::default();
    macro_rules! field {
        ($name:ident,$scope:expr) => {
            facts.contact.$name = select(
                reports
                    .iter()
                    .map(|(o, s)| (*o, *s, &o.facts.contact.$name))
                    .collect(),
                $scope,
                concat!("contact.", stringify!($name)),
                view,
                &mut uncertain,
                &mut current_supporting,
                &mut superseded,
            )?;
        };
    }
    field!(own_call, false);
    field!(worked_call, false);
    field!(start, false);
    field!(band, false);
    field!(receive_band, false);
    field!(frequency, false);
    field!(receive_frequency, false);
    field!(mode, false);
    let mut confirmation_supporting = std::collections::BTreeSet::new();
    facts.contact.confirmation = select(
        reports
            .iter()
            .map(|(o, s)| (*o, *s, &o.facts.contact.confirmation))
            .collect(),
        true,
        "contact.confirmation",
        view,
        &mut uncertain,
        &mut confirmation_supporting,
        &mut superseded,
    )?;
    current_supporting.extend(&confirmation_supporting);
    facts.own_station = select_station(
        &reports,
        false,
        view,
        &mut uncertain,
        &mut current_supporting,
        &mut superseded,
    )?;
    if reports.iter().any(|(o, _)| o.facts.counterpart.is_some()) {
        facts.counterpart = Some(select_station(
            &reports,
            true,
            view,
            &mut uncertain,
            &mut current_supporting,
            &mut superseded,
        )?);
    }
    let ordering = ProviderOrdering {
        qso: reports.iter().filter_map(|(o, _)| o.ordering.qso).max(),
        confirmation: reports
            .iter()
            .filter_map(|(o, _)| o.ordering.confirmation)
            .max(),
    };
    Ok(SelectedObservations {
        current: EstablishedObservation {
            identity: RemoteIdentity::Complete(input.external.clone()),
            facts,
            ordering,
        },
        supporting: reports.iter().map(|(o, _)| o.id).collect(),
        current_supporting: current_supporting
            .difference(&superseded)
            .copied()
            .collect(),
        confirmation_supporting: confirmation_supporting.into_iter().collect(),
        uncertain,
    })
}

/// Select actual station fields with the appropriate provider update scope.
fn select_station(
    reports: &[(&RemoteObservation, &ServiceSupport)],
    counterpart: bool,
    view: &ServiceView<'_>,
    uncertain: &mut Vec<String>,
    supporting: &mut std::collections::BTreeSet<ObservationId>,
    superseded: &mut std::collections::BTreeSet<ObservationId>,
) -> anyhow::Result<StationReport> {
    let mut station = StationReport::default();
    macro_rules! item {
        ($name:ident) => {
            let values = reports
                .iter()
                .filter_map(|(o, s)| {
                    let report = if counterpart {
                        o.facts.counterpart.as_ref()
                    } else {
                        Some(&o.facts.own_station)
                    };
                    report.map(|r| (*o, *s, &r.$name))
                })
                .collect();
            station.$name = select(
                values,
                counterpart,
                &format!(
                    "{}.{name}",
                    if counterpart {
                        "counterpart"
                    } else {
                        "own_station"
                    },
                    name = stringify!($name)
                ),
                view,
                uncertain,
                supporting,
                superseded,
            )?;
        };
    }
    item!(name);
    item!(position);
    item!(dxcc);
    item!(primary_subdivision);
    item!(secondary_subdivision);
    item!(cq_zone);
    item!(itu_zone);
    item!(references);
    Ok(station)
}
