//! Coordinated provider reads, semantic observation transitions and durable coverage.

use super::{
    ExternalBindingObservation, ServiceRecord, ServiceSupport, ServiceView, SupportRequirement,
    VersionReference,
    combination::{ObservationSelectionInput, select_observations},
    operation::{CoupledCommit, ServiceDecision, plan_changes},
};
use crate::{
    account::{AccountMetadata, EvidenceView},
    membership::MembershipSnapshot,
    record::{AcceptedRecord, Observation, RecordId, RecordKey, RecordState, RecordValue},
};
use clacks_model::{
    ids::{AcquisitionId, DispositionId, ObservationId, OperationId},
    scalar::{UtcSecond, UtcTimestamp},
    service::{
        destination::Destination,
        evidence::{
            ServicePayload, ServiceRecordId,
            download::{
                AcquisitionPhase, AcquisitionState, DownloadControl, StreamProgress, same_stream,
            },
            observation::{
                ObservationComparison, RemoteIdentity, RemoteObservation, Reported, StationReport,
                compare,
            },
            processing::{AssociationState, ProcessingDecision, ProcessingDisposition},
        },
    },
};

/// Consistent account read view used by a real download worker transaction.
pub struct DownloadContext<'a> {
    /// Stable transition identity.
    pub id: OperationId,
    /// Current account head.
    pub account: &'a AccountMetadata,
    /// Current authorized membership.
    pub membership: &'a MembershipSnapshot,
    /// Explicit record reads, including absence for new rows.
    pub reads: &'a [Observation],
    /// Unique binding-index observations.
    pub bindings: &'a [ExternalBindingObservation],
    /// Public destination descriptors.
    pub destinations: &'a [Destination],
    /// Retained timeline/baseline interpretation.
    pub evidence: &'a EvidenceView<'a>,
}

/// New request intent sharing one destination's coordination row.
pub struct DownloadStartInput<'a> {
    /// Consistent source observations.
    pub context: &'a DownloadContext<'a>,
    /// Compared empty/available destination control.
    pub control: &'a DownloadControl,
    /// New request identity.
    pub id: AcquisitionId,
    /// Exact public request parameters.
    pub query: &'a clacks_model::service::evidence::observation::QueryIdentity,
    /// Caller-supplied diagnostic start time.
    pub at: UtcTimestamp,
}

/// Plan a serialized request start; the actual provider call occurs after commit.
#[tracing::instrument(
    skip_all,
    name = "sync.download.start.plan",
    level = "debug",
    fields(
        account_id = %input.context.account.account,
        operation_id = %input.context.id,
        destination_id = %input.control.destination,
        acquisition_id = %input.id,
    ),
)]
pub fn plan_start(
    input: &DownloadStartInput<'_>,
) -> anyhow::Result<ServiceDecision<CoupledCommit>> {
    input.control.validate()?;
    anyhow::ensure!(
        input.query.destination == input.control.destination,
        "query/control destination differs"
    );
    validate_control(input.context, input.control)?;
    if input.control.active.is_some() {
        return Ok(ServiceDecision::Review(
            "a request is already active for this destination".into(),
        ));
    }
    let mut control = input.control.clone();
    control.active = Some(input.id);
    let acquisition = AcquisitionState {
        id: input.id,
        query: input.query.clone(),
        phase: AcquisitionPhase::Active,
        started_at: input.at,
        retained_inputs: Default::default(),
    };
    group(
        input.context,
        vec![
            ServicePayload::Acquisition(acquisition),
            ServicePayload::Download(control),
        ],
    )
}

/// A fresh parsed provider response with new occurrence/disposition identities.
pub struct ObservationInput<'a> {
    /// Current real transaction's source view.
    pub context: &'a DownloadContext<'a>,
    /// Original request that obtained this response, even when late.
    pub acquisition: &'a AcquisitionState,
    /// New immutable occurrence proposed by response processing.
    pub observation: &'a RemoteObservation,
    /// New durable processing-state identity, if a semantic occurrence is retained.
    pub disposition: DispositionId,
}

/// Retain meaningful new provider evidence without turning recovery/replays into fresh reads.
#[tracing::instrument(
    skip_all,
    name = "sync.download.observation.plan",
    level = "debug",
    fields(
        account_id = %input.context.account.account,
        operation_id = %input.context.id,
        destination_id = %input.observation.destination,
        acquisition_id = %input.acquisition.id,
        observation_id = %input.observation.id,
    ),
)]
pub fn plan_observation(
    input: &ObservationInput<'_>,
) -> anyhow::Result<ServiceDecision<CoupledCommit>> {
    let ctx = input.context;
    let incoming = input.observation;
    let acquisition = source(ctx, ServiceRecordId::Acquisition(input.acquisition.id))?;
    let Some(acq_record) = super::payload(acquisition) else {
        anyhow::bail!("acquisition source is not service state")
    };
    anyhow::ensure!(
        matches!(&acq_record.payload,ServicePayload::Acquisition(actual) if actual.id==input.acquisition.id && actual.query==input.acquisition.query && actual.started_at==input.acquisition.started_at),
        "acquisition source identity/query differs"
    );
    anyhow::ensure!(
        incoming.acquisition.id == input.acquisition.id
            && incoming.acquisition.query == input.acquisition.query
            && incoming.destination == input.acquisition.query.destination,
        "response provenance differs from original request"
    );
    ServicePayload::Observation(Box::new(incoming.clone())).validate()?;
    anyhow::ensure!(
        !has_clear(incoming),
        "provider clear semantics are not established for this response"
    );
    let incoming_key = RecordKey {
        account: ctx.account.account,
        id: RecordId::Service(ServiceRecordId::Observation(incoming.id)),
    };
    if ctx
        .reads
        .iter()
        .any(|r| r.key() == incoming_key && matches!(r, Observation::Accepted(_)))
    {
        anyhow::bail!(
            "accepted observation replay requires recovery, not fresh response processing"
        );
    }
    let rows = accepted(ctx.reads);
    let mut decision = ProcessingDecision::PendingReview;
    let mut association = None;
    if let RemoteIdentity::Complete(external) = &incoming.identity {
        let view = ServiceView {
            account: ctx.account.account,
            records: &rows,
            bindings: ctx.bindings,
            evidence: ctx.evidence,
        };
        let selected = select_observations(&ObservationSelectionInput {
            external,
            view: &view,
        })?;
        if !selected.supporting.is_empty() {
            match compare(&selected.current, incoming)? {
                ObservationComparison::Unchanged => return Ok(ServiceDecision::Unchanged),
                ObservationComparison::OrderingOnly => {
                    if let Some(old) = current_disposition(&rows, &selected)? {
                        decision = old.decision;
                        association = old.association;
                    }
                }
                _ => {}
            }
        }
        for s in rows.iter().filter_map(super::payload) {
            match &s.payload {
                ServicePayload::Suppression(s) if &s.external == external => {
                    decision = ProcessingDecision::Suppressed;
                    association = None;
                }
                ServicePayload::Association(a) if &a.external == external => {
                    if a.state != AssociationState::Active {
                        decision = ProcessingDecision::Suppressed;
                        association = None;
                    } else if decision != ProcessingDecision::Suppressed {
                        decision = ProcessingDecision::Associated;
                        association = Some(a.id);
                    }
                }
                _ => {}
            }
        }
    }
    let disposition = ProcessingDisposition {
        id: input.disposition,
        observation: incoming.id,
        decision,
        association,
    };
    let record = ServiceRecord {
        payload: ServicePayload::Observation(Box::new(incoming.clone())),
        support: ServiceSupport {
            operation: Some(ctx.id),
            acquisition: Some(
                acq_record
                    .support
                    .acquisition
                    .clone()
                    .unwrap_or_else(|| acquisition.stamp.clone()),
            ),
            destination: ctx
                .destinations
                .iter()
                .find(|d| d.id == incoming.destination)
                .cloned(),
            ..Default::default()
        },
    };
    // Carry the original request-start context directly; the response acceptance is not freshness.
    let mut changes = Vec::new();
    let key = incoming_key;
    let Some(current) = ctx.reads.iter().find(|r| r.key() == key) else {
        return Ok(ServiceDecision::Incomplete(
            "new observation absence read missing".into(),
        ));
    };
    changes.push(super::operation::NativeChange::Server(
        crate::acceptance::ServerChange {
            key,
            state: RecordState::Live(RecordValue::Service(Box::new(record))),
            expected: crate::client::VersionCondition::from_observation(current)?,
            dependencies: vec![],
        },
    ));
    plan_changes(
        ctx.id,
        ctx.account,
        ctx.membership,
        ctx.reads,
        ctx.bindings,
        ctx.destinations,
        ctx.evidence,
        changes,
        vec![ServicePayload::Disposition(disposition)],
        vec![],
    )
}

/// Explicit request completion/failure and its response-retention evidence.
pub struct DownloadProgressInput<'a> {
    /// Current consistent transaction view.
    pub context: &'a DownloadContext<'a>,
    /// Compared destination coordination row.
    pub control: &'a DownloadControl,
    /// Compared original request state.
    pub acquisition: &'a AcquisitionState,
    /// Retained occurrence IDs covering every processed response item.
    pub retained_inputs: &'a [ObservationId],
    /// True only after all requested pages/windows were successfully decoded.
    pub response_complete: bool,
    /// Optional failure/cancellation diagnostic, preventing progress advance.
    pub failure: Option<String>,
    /// Provider change watermark supplied by successful query semantics.
    pub watermark: Option<UtcSecond>,
    /// Caller-supplied explicit close time.
    pub at: UtcTimestamp,
}

/// Close a request with durable input accounting before advancing its exact stream watermark.
#[tracing::instrument(
    skip_all,
    name = "sync.download.progress.plan",
    level = "debug",
    fields(
        account_id = %input.context.account.account,
        operation_id = %input.context.id,
        destination_id = %input.control.destination,
        acquisition_id = %input.acquisition.id,
    ),
)]
pub fn plan_progress(
    input: &DownloadProgressInput<'_>,
) -> anyhow::Result<ServiceDecision<CoupledCommit>> {
    let ctx = input.context;
    validate_control(ctx, input.control)?;
    let acq = source(ctx, ServiceRecordId::Acquisition(input.acquisition.id))?;
    anyhow::ensure!(
        super::payload(acq)
            .is_some_and(|r| r.payload == ServicePayload::Acquisition(input.acquisition.clone())),
        "acquisition changed since processing"
    );
    anyhow::ensure!(
        input.control.destination == input.acquisition.query.destination,
        "completion crosses destination"
    );
    if input.control.active != Some(input.acquisition.id)
        || input.acquisition.phase != AcquisitionPhase::Active
    {
        return Ok(ServiceDecision::Review(
            "request no longer owns destination control; late response cannot advance progress"
                .into(),
        ));
    }
    let mut requirements = Vec::new();
    for id in input.retained_inputs {
        let row = source(ctx, ServiceRecordId::Observation(*id))?;
        let Some(s) = super::payload(row) else {
            anyhow::bail!("retained input is not an observation")
        };
        let ServicePayload::Observation(obs) = &s.payload else {
            anyhow::bail!("retained input has incorrect type")
        };
        anyhow::ensure!(
            obs.destination == input.control.destination,
            "retained input crosses destination"
        );
        let disposition = ctx
            .reads
            .iter()
            .filter_map(|r| {
                if let Observation::Accepted(v) = r {
                    Some(v)
                } else {
                    None
                }
            })
            .find(|row| {
                super::payload(row).is_some_and(
                    |s| matches!(&s.payload,ServicePayload::Disposition(d) if d.observation==*id),
                )
            });
        let Some(d) = disposition else {
            return Ok(ServiceDecision::Incomplete(format!(
                "input {id} has no durable processing state"
            )));
        };
        requirements.push(SupportRequirement::ExactFact(VersionReference::from(row)));
        requirements.push(SupportRequirement::CoveredEffect(VersionReference::from(d)));
    }
    let complete = input.response_complete && input.failure.is_none();
    let mut acquisition = input.acquisition.clone();
    acquisition
        .retained_inputs
        .extend(input.retained_inputs.iter().copied());
    acquisition.phase = if complete {
        AcquisitionPhase::Completed { at: input.at }
    } else {
        AcquisitionPhase::Failed {
            at: input.at,
            reason: input
                .failure
                .clone()
                .unwrap_or_else(|| "incomplete response".into()),
        }
    };
    let mut control = input.control.clone();
    control.active = None;
    if complete {
        let old = control
            .progress
            .iter()
            .find(|p| same_stream(&p.query, &acquisition.query))
            .and_then(|p| p.watermark);
        if let (Some(old), Some(next)) = (old, input.watermark) {
            anyhow::ensure!(next >= old, "provider change watermark regressed");
        }
        control
            .progress
            .retain(|p| !same_stream(&p.query, &acquisition.query));
        control.progress.push(StreamProgress {
            query: acquisition.query.clone(),
            watermark: input.watermark.or(old),
        });
    }
    let mut support = super::payload(acq)
        .map(|s| s.support.clone())
        .ok_or_else(|| anyhow::anyhow!("acquisition carrier missing"))?;
    support.operation = Some(ctx.id);
    support.acquisition = Some(
        support
            .acquisition
            .clone()
            .unwrap_or_else(|| acq.stamp.clone()),
    );
    support.required = requirements;
    let change = super::operation::NativeChange::Server(crate::acceptance::ServerChange {
        key: acq.key,
        state: RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Acquisition(acquisition),
            support,
        }))),
        expected: crate::client::VersionCondition::from_observation(&Observation::Accepted(
            acq.clone(),
        ))?,
        dependencies: vec![],
    });
    let plan = plan_changes(
        ctx.id,
        ctx.account,
        ctx.membership,
        ctx.reads,
        ctx.bindings,
        ctx.destinations,
        ctx.evidence,
        vec![change],
        vec![ServicePayload::Download(control)],
        vec![],
    );
    if matches!(&plan, Ok(ServiceDecision::Ready(_))) {
        tracing::debug!(
            event = "download.closure.planned",
            account_id = %ctx.account.account,
            operation_id = %ctx.id,
            acquisition_id = %input.acquisition.id,
            destination_id = %input.control.destination,
            acquisition_outcome = if complete { "complete" } else { "failed" },
        );
    }
    plan
}

/// Build ordinary coupled acceptance with all original source guards.
fn group(
    ctx: &DownloadContext<'_>,
    payloads: Vec<ServicePayload>,
) -> anyhow::Result<ServiceDecision<CoupledCommit>> {
    plan_changes(
        ctx.id,
        ctx.account,
        ctx.membership,
        ctx.reads,
        ctx.bindings,
        ctx.destinations,
        ctx.evidence,
        vec![],
        payloads,
        vec![],
    )
}

/// Borrow a required accepted service row from the explicit read set.
fn source<'a>(
    ctx: &DownloadContext<'a>,
    id: ServiceRecordId,
) -> anyhow::Result<&'a AcceptedRecord> {
    let key = RecordKey {
        account: ctx.account.account,
        id: RecordId::Service(id),
    };
    ctx.reads
        .iter()
        .find_map(|r| {
            if let Observation::Accepted(v) = r
                && v.key == key
            {
                Some(v)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("required source {key:?} not available"))
}

/// Reject caller-supplied control values that differ from the real transactional read.
fn validate_control(ctx: &DownloadContext<'_>, control: &DownloadControl) -> anyhow::Result<()> {
    let key = RecordKey {
        account: ctx.account.account,
        id: RecordId::Service(ServiceRecordId::Download(control.id)),
    };
    match ctx.reads.iter().find(|r| r.key() == key) {
        Some(Observation::Accepted(v)) => anyhow::ensure!(
            super::payload(v)
                .is_some_and(|s| s.payload == ServicePayload::Download(control.clone())),
            "control changed since read"
        ),
        Some(Observation::Absent { .. }) => anyhow::ensure!(
            control.active.is_none() && control.progress.is_empty(),
            "absent control cannot have prior state"
        ),
        _ => anyhow::bail!("explicit control read required"),
    }
    Ok(())
}

/// Gather accepted interpretation rows without altering original acceptance.
fn accepted(reads: &[Observation]) -> Vec<AcceptedRecord> {
    reads
        .iter()
        .filter_map(|r| {
            if let Observation::Accepted(v) = r {
                Some(v.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Detect unsupported explicit clears in every reported provider field.
fn has_clear(v: &RemoteObservation) -> bool {
    let c = &v.facts.contact;
    matches!(c.own_call, Reported::Clear)
        || matches!(c.worked_call, Reported::Clear)
        || matches!(c.start, Reported::Clear)
        || matches!(c.band, Reported::Clear)
        || matches!(c.receive_band, Reported::Clear)
        || matches!(c.frequency, Reported::Clear)
        || matches!(c.receive_frequency, Reported::Clear)
        || matches!(c.mode, Reported::Clear)
        || matches!(c.confirmation, Reported::Clear)
        || station_clear(&v.facts.own_station)
        || v.facts.counterpart.as_ref().is_some_and(station_clear)
}

/// Check optional station reports without filling missing values from another source.
fn station_clear(s: &StationReport) -> bool {
    matches!(s.name, Reported::Clear)
        || matches!(s.position, Reported::Clear)
        || matches!(s.dxcc, Reported::Clear)
        || matches!(s.primary_subdivision, Reported::Clear)
        || matches!(s.secondary_subdivision, Reported::Clear)
        || matches!(s.cq_zone, Reported::Clear)
        || matches!(s.itu_zone, Reported::Clear)
        || matches!(s.references, Reported::Clear)
}

/// Reuse a decision only when its reviewed occurrence still represents established facts.
fn current_disposition<'a>(
    rows: &'a [AcceptedRecord],
    selected: &super::combination::SelectedObservations,
) -> anyhow::Result<Option<&'a ProcessingDisposition>> {
    if !selected.uncertain.is_empty() {
        return Ok(None);
    }
    let mut current: Option<&ProcessingDisposition> = None;
    for record in rows.iter().filter_map(super::payload) {
        let ServicePayload::Disposition(disposition) = &record.payload else {
            continue;
        };
        if !selected
            .current_supporting
            .contains(&disposition.observation)
        {
            continue;
        }
        let observation = rows
            .iter()
            .filter_map(super::payload)
            .find_map(|s| match &s.payload {
                ServicePayload::Observation(o) if o.id == disposition.observation => Some(o),
                _ => None,
            });
        let Some(observation) = observation else {
            continue;
        };
        if !matches!(
            compare(&selected.current, observation)?,
            ObservationComparison::Unchanged | ObservationComparison::OrderingOnly
        ) {
            continue;
        }
        if current.is_some_and(|old| {
            old.decision != disposition.decision || old.association != disposition.association
        }) {
            return Ok(None);
        }
        current = Some(disposition);
    }
    Ok(current)
}
