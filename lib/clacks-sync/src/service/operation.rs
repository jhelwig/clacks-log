//! Conditional multi-record service operations built on ordinary acceptance.

use super::{
    ExternalBindingGuard, ExternalBindingObservation, ServiceRecord, ServiceSupport,
    SupportRequirement, VersionReference,
};
use crate::{
    acceptance::{
        AcceptanceDecision, AcceptanceInput, JournalChange, ServerChange, plan_acceptance,
        plan_server_acceptance,
    },
    account::{AccountMetadata, EvidenceView},
    client::{PreparedChange, VersionCondition},
    membership::MembershipSnapshot,
    record::{
        AcceptedRecord, Observation, Position, RecordId, RecordKey, RecordState, RecordValue,
    },
};
use clacks_model::{
    ids::{AttemptId, ObservationId, OperationId, QsoId, SubmissionResultId},
    service::{
        destination::Destination,
        evidence::{
            ImportedAssertion, ServicePayload, ServiceRecordId,
            processing::{
                AssociationState, DeletionSuppression, ExternalAssociation, ProcessingDecision,
                ProcessingDisposition,
            },
            submission::{SubmissionAttempt, SubmissionOutcome, SubmissionResult},
        },
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Outcome of a pure planner with no external network or storage side effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceDecision<T> {
    /// Commit the returned instructions together under their conditions.
    Ready(T),
    /// No new semantic mutation is required.
    Unchanged,
    /// Explicit operator resolution is required.
    Review(String),
    /// Required durable input is unavailable.
    Incomplete(String),
    /// A previously compared row no longer matches.
    Stale(Observation),
}

/// Existing native acceptance request, retaining its original author/admission semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeChange {
    /// Service-produced or explicitly approved server values.
    Server(ServerChange),
    /// Actual client proposal with its checked causal contribution.
    Client(PreparedChange),
}

impl NativeChange {
    /// Identify the record whose original guard must hold.
    fn key(&self) -> RecordKey {
        match self {
            Self::Server(c) => c.key,
            Self::Client(c) => c.key,
        }
    }
    /// Borrow the native value without introducing another QSO representation.
    fn state(&self) -> &RecordState {
        match self {
            Self::Server(c) => &c.state,
            Self::Client(c) => &c.state,
        }
    }
    /// Borrow the target compare condition.
    fn expected(&self) -> &VersionCondition {
        match self {
            Self::Server(c) => &c.expected,
            Self::Client(c) => &c.expected,
        }
    }
    /// Borrow explicit source-reference conditions.
    fn dependencies(&self) -> &[VersionCondition] {
        match self {
            Self::Server(c) => &c.dependencies,
            Self::Client(c) => &c.dependencies,
        }
    }
}

/// Related native/evidence work that must not become partially durable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceOperation {
    /// Save validated import contents and imported status baselines together.
    ApplyImport {
        /// Approved contact create/update.
        qso_change: NativeChange,
        /// Positive independent service claims.
        assertions: Vec<ImportedAssertion>,
        /// Any explicitly approved related tag changes.
        selected_tag_effects: Vec<NativeChange>,
        /// Selected external binding, when one exists.
        association: Option<ExternalAssociation>,
    },
    /// Record a reviewed observation disposition with all accepted effects.
    CompleteObservation {
        /// Exact occurrence shown to the operator.
        observation: ObservationId,
        /// Durable action or remaining review state.
        disposition: ProcessingDisposition,
        /// New/updated binding when selected.
        association: Option<ExternalAssociation>,
        /// Explicit add/correction, if approved.
        qso_change: Option<NativeChange>,
        /// Explicit candidate choice; absence permits only an established/unique confident match.
        selected: Option<QsoId>,
    },
    /// Delete a contact while retaining every known external relationship/suppression.
    DeleteQso {
        /// Approved contact deletion.
        qso_change: NativeChange,
        /// All retained bindings affected by this deletion.
        associations: Vec<ExternalAssociation>,
        /// Deletion memory for each known external identity.
        suppression: Vec<DeletionSuppression>,
    },
    /// Retain frozen attempted values before dispatch outside the transaction.
    PrepareSubmission {
        /// Intent alone does not establish sending.
        attempt: SubmissionAttempt,
    },
    /// Retain the provider result and actual job completion instructions together.
    RecordSubmissionResult {
        /// Scope-qualified outcome evidence.
        result: SubmissionResult,
        /// Exact members supplied by the actual request journal.
        batch_members: Vec<AttemptId>,
        /// Authoritatively established external binding, if safe.
        association: Option<ExternalAssociation>,
    },
}

/// Recognition supplied only when an unassociated downloaded record needs matching.
pub struct MatchContext<'a> {
    /// Current band/mode interpretation catalog.
    pub recognition: &'a clacks_model::recognition::RecognitionCatalog,
    /// Source-derived LoTW mode mappings.
    pub lotw: &'a clacks_model::service::lotw::LotwConfiguration,
}

/// Consistent real-store read set used to construct one atomic operation.
pub struct ServiceOperationInput<'a> {
    /// Stable durable operation identity, reused for retries/recovery.
    pub id: OperationId,
    /// Current account head.
    pub account: &'a AccountMetadata,
    /// Current admission generation and enrollments.
    pub membership: &'a MembershipSnapshot,
    /// Explicit source observations, including certified absence for new identities.
    pub reads: &'a [Observation],
    /// Actual unique-index observations.
    pub bindings: &'a [ExternalBindingObservation],
    /// Public retained namespace descriptions, never credentials.
    pub destinations: &'a [Destination],
    /// Current causal interpretation.
    pub evidence: &'a EvidenceView<'a>,
    /// Chosen related work.
    pub operation: &'a ServiceOperation,
    /// Interpretation data required by automatic matching.
    pub matching: Option<MatchContext<'a>>,
}

/// Durable descriptor whose members must be represented before acknowledging its operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoupledRequirement {
    /// Stable atomic-unit identity.
    pub id: OperationId,
    /// Accepted members; later established successors may cover effects.
    pub members: Vec<VersionReference>,
}

/// Semantic outcome to commit beside the actual worker's job row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobOutcomeUpdate {
    /// Attempt whose request completed or became uncertain.
    pub attempt: AttemptId,
    /// Retained result establishing the outcome.
    pub result: SubmissionResultId,
    /// Scope-qualified outcome, not a new queue state machine.
    pub outcome: SubmissionOutcome,
}

/// One indivisible real-store transaction; intermediate account heads are not checkpoints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoupledCommit {
    /// Original head required at commit.
    pub expected_head: Position,
    /// Original admission generation required at commit.
    pub expected_membership_generation: u64,
    /// Original target/reference conditions, including observations of absence.
    pub conditions: Vec<VersionCondition>,
    /// Unique-index conditions enforced atomically with the rows.
    pub external_guards: Vec<ExternalBindingGuard>,
    /// Final accepted account coverage.
    pub next_account: AccountMetadata,
    /// Rows to save atomically.
    pub accepted: Vec<AcceptedRecord>,
    /// Existing transfer-journal changes to save atomically.
    pub journals: Vec<JournalChange>,
    /// Exact atomic-unit descriptor to retain/transfer with its members.
    pub unit: CoupledRequirement,
    /// Actual worker job updates to include in the same database transaction.
    pub job_updates: Vec<JobOutcomeUpdate>,
}

/// Plan related native mutations and evidence without sending bytes or committing intermediate rows.
pub fn plan_operation(
    input: &ServiceOperationInput<'_>,
) -> anyhow::Result<ServiceDecision<CoupledCommit>> {
    input.membership.validate(input.account)?;
    let mut changes = Vec::new();
    let mut payloads = Vec::new();
    let mut job_updates = Vec::new();
    match input.operation {
        ServiceOperation::ApplyImport {
            qso_change,
            assertions,
            selected_tag_effects,
            association,
        } => {
            let qso = qso_id(qso_change)?;
            anyhow::ensure!(
                matches!(qso_change.state(), RecordState::Live(RecordValue::Qso(_))),
                "import must save a valid contact"
            );
            for a in assertions {
                anyhow::ensure!(a.qso == qso, "import assertion targets another contact");
                payloads.push(ServicePayload::Imported(Box::new(a.clone())));
            }
            changes.extend(selected_tag_effects.iter().cloned());
            changes.push(qso_change.clone());
            if let Some(a) = association {
                anyhow::ensure!(a.qso == qso, "import binding targets another contact");
                payloads.push(ServicePayload::Association(a.clone()));
            }
        }
        ServiceOperation::CompleteObservation {
            observation,
            disposition,
            association,
            qso_change,
            selected,
        } => {
            anyhow::ensure!(
                *observation == disposition.observation,
                "reviewed observation differs from disposition"
            );
            let key = RecordKey {
                account: input.account.account,
                id: RecordId::Service(ServiceRecordId::Observation(*observation)),
            };
            let Some(Observation::Accepted(source)) = input.reads.iter().find(|r| r.key() == key)
            else {
                return Ok(ServiceDecision::Incomplete(
                    "reviewed observation missing".into(),
                ));
            };
            let Some(s) = super::payload(source) else {
                anyhow::bail!("reviewed source is not service evidence")
            };
            let ServicePayload::Observation(obs) = &s.payload else {
                anyhow::bail!("reviewed source is not an observation")
            };
            if matches!(
                disposition.decision,
                ProcessingDecision::Added | ProcessingDecision::Associated
            ) {
                let Some(a) = association else {
                    return Ok(ServiceDecision::Incomplete(
                        "completed decision requires a binding".into(),
                    ));
                };
                anyhow::ensure!(
                    disposition.association == Some(a.id),
                    "disposition binding differs from approved binding"
                );
                if let clacks_model::service::evidence::observation::RemoteIdentity::Complete(k) =
                    &obs.identity
                {
                    anyhow::ensure!(
                        *k == a.external,
                        "binding differs from observation identity"
                    );
                } else {
                    return Ok(ServiceDecision::Review(
                        "external identity incomplete".into(),
                    ));
                }
                if disposition.decision == ProcessingDecision::Associated {
                    let established = input.bindings.iter().any(|b| {
                        b.external == a.external
                            && b.account == input.account.account
                            && b.binding.as_ref().is_some_and(|v| {
                                v.qso == a.qso && v.state == AssociationState::Active
                            })
                    });
                    if !established {
                        let Some(context) = &input.matching else {
                            return Ok(ServiceDecision::Incomplete(
                                "matching interpretation missing".into(),
                            ));
                        };
                        let rows = accepted_reads(input.reads);
                        let candidates = super::association::classify_candidates(
                            &super::association::CandidateInput {
                                observation: obs,
                                account: input.account.account,
                                account_qsos: &rows,
                                recognition: context.recognition,
                                lotw_configuration: context.lotw,
                            },
                        )?;
                        let valid = if let Some(choice) = selected {
                            *choice == a.qso
                                && (candidates.confident.contains(choice)
                                    || candidates.possible.iter().any(|c| c.qso == *choice))
                        } else {
                            candidates.confident.len() == 1 && candidates.confident.contains(&a.qso)
                        };
                        if !valid {
                            return Ok(ServiceDecision::Review(
                                "explicit selection of a compatible candidate required".into(),
                            ));
                        }
                    }
                } else {
                    anyhow::ensure!(
                        qso_change
                            .as_ref()
                            .is_some_and(|c| qso_id(c).is_ok_and(|q| q == a.qso)
                                && matches!(c.expected(), VersionCondition::Absent { .. })),
                        "adding a contact requires an explicit new contact change"
                    );
                }
            }
            if let Some(c) = qso_change {
                changes.push(c.clone());
            }
            if let Some(a) = association {
                payloads.push(ServicePayload::Association(a.clone()));
            }
            payloads.push(ServicePayload::Disposition(disposition.clone()));
        }
        ServiceOperation::DeleteQso {
            qso_change,
            associations,
            suppression,
        } => {
            let qso = qso_id(qso_change)?;
            anyhow::ensure!(
                matches!(qso_change.state(), RecordState::Deleted),
                "delete operation requires a QSO deletion"
            );
            let known = accepted_reads(input.reads);
            for old in known
                .iter()
                .filter_map(super::payload)
                .filter_map(|s| {
                    if let ServicePayload::Association(a) = &s.payload {
                        Some(a)
                    } else {
                        None
                    }
                })
                .filter(|a| a.qso == qso && a.state == AssociationState::Active)
            {
                anyhow::ensure!(
                    associations.iter().any(|a| a.id == old.id
                        && a.qso == qso
                        && a.external == old.external
                        && a.state == AssociationState::DeletedQso),
                    "deletion omits a known active binding"
                );
            }
            for a in associations {
                anyhow::ensure!(
                    a.qso == qso
                        && a.state == AssociationState::DeletedQso
                        && suppression
                            .iter()
                            .any(|s| s.qso == qso && s.external == a.external),
                    "deletion needs matched binding and suppression"
                );
                payloads.push(ServicePayload::Association(a.clone()));
            }
            for s in suppression {
                anyhow::ensure!(s.qso == qso, "suppression targets another contact");
                payloads.push(ServicePayload::Suppression(s.clone()));
            }
            changes.push(qso_change.clone());
        }
        ServiceOperation::PrepareSubmission { attempt } => {
            let key = RecordKey {
                account: input.account.account,
                id: RecordId::Qso(attempt.qso),
            };
            if !input.reads.iter().any(|r| {
                r.key() == key
                    && matches!(
                        r,
                        Observation::Accepted(AcceptedRecord {
                            state: RecordState::Live(RecordValue::Qso(_)),
                            ..
                        })
                    )
            }) {
                return Ok(ServiceDecision::Incomplete(
                    "submission contact not live".into(),
                ));
            }
            payloads.push(ServicePayload::Attempt(attempt.clone()));
        }
        ServiceOperation::RecordSubmissionResult {
            result,
            batch_members,
            association,
        } => {
            let actual: BTreeSet<_> = result.scope.attempts().into_iter().collect();
            anyhow::ensure!(
                actual == batch_members.iter().copied().collect(),
                "result members differ from actual request members"
            );
            for id in actual {
                job_updates.push(JobOutcomeUpdate {
                    attempt: id,
                    result: result.id,
                    outcome: result.outcome.clone(),
                });
            }
            payloads.push(ServicePayload::Result(result.clone()));
            if let Some(a) = association {
                anyhow::ensure!(
                    result.external.as_ref() == Some(&a.external),
                    "binding not established by returned identity"
                );
                let Some(member) = result.identified_attempt()? else {
                    return Ok(ServiceDecision::Review(format!(
                        "result {} external identity lacks member-level correlation",
                        result.id
                    )));
                };
                let Some(attempt) = input
                    .reads
                    .iter()
                    .filter_map(|r| {
                        if let Observation::Accepted(row) = r {
                            super::payload(row)
                        } else {
                            None
                        }
                    })
                    .find_map(|s| {
                        if let ServicePayload::Attempt(v) = &s.payload
                            && v.id == member
                        {
                            Some(v)
                        } else {
                            None
                        }
                    })
                else {
                    return Ok(ServiceDecision::Incomplete(format!(
                        "result {} attempt {member} is missing",
                        result.id
                    )));
                };
                anyhow::ensure!(
                    attempt.qso == a.qso && attempt.projection.destination() == result.destination,
                    "result {} attempt {member} belongs to QSO {}, not binding QSO {}",
                    result.id,
                    attempt.qso,
                    a.qso
                );
                payloads.push(ServicePayload::Association(a.clone()));
            }
        }
    }
    plan_changes(
        input.id,
        input.account,
        input.membership,
        input.reads,
        input.bindings,
        input.destinations,
        input.evidence,
        changes,
        payloads,
        job_updates,
    )
}

/// Resolve a contact change identity without accepting a different native kind.
fn qso_id(change: &NativeChange) -> anyhow::Result<QsoId> {
    if let RecordId::Qso(id) = change.key().id {
        Ok(id)
    } else {
        anyhow::bail!("operation requires a contact change")
    }
}

/// Copy accepted source rows for interpretation while preserving their original contexts.
fn accepted_reads(reads: &[Observation]) -> Vec<AcceptedRecord> {
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

/// Plan an internal dependency-ordered group through the ordinary admission paths.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_changes(
    id: OperationId,
    account: &AccountMetadata,
    membership: &MembershipSnapshot,
    reads: &[Observation],
    bindings: &[ExternalBindingObservation],
    destinations: &[Destination],
    evidence: &EvidenceView<'_>,
    mut changes: Vec<NativeChange>,
    payloads: Vec<ServicePayload>,
    job_updates: Vec<JobOutcomeUpdate>,
) -> anyhow::Result<ServiceDecision<CoupledCommit>> {
    let mut keys = BTreeSet::new();
    for r in reads {
        r.validate()?;
        anyhow::ensure!(
            r.key().account == account.account && keys.insert(r.key()),
            "duplicate or cross-account source read"
        );
    }
    for b in bindings {
        anyhow::ensure!(b.account == account.account, "binding read crosses account");
    }
    clacks_model::service::destination::validate_destinations(destinations)?;
    for payload in payloads {
        let key = RecordKey {
            account: account.account,
            id: RecordId::Service(payload.id()),
        };
        let Some(source) = reads.iter().find(|r| r.key() == key) else {
            return Ok(ServiceDecision::Incomplete(format!(
                "explicit source observation required for {key:?}"
            )));
        };
        let support = if let Observation::Accepted(old) = source
            && let Some(old) = super::payload(old)
            && old.payload == payload
        {
            old.support.clone()
        } else {
            ServiceSupport {
                operation: Some(id),
                destination: payload
                    .destination()
                    .and_then(|d| destinations.iter().find(|v| v.id == d).cloned()),
                ..Default::default()
            }
        };
        changes.push(NativeChange::Server(ServerChange {
            key,
            state: RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                payload,
                support,
            }))),
            expected: VersionCondition::from_observation(source)?,
            dependencies: vec![],
        }));
    }
    let mut writes = BTreeSet::new();
    let mut conditions = Vec::new();
    for c in &changes {
        anyhow::ensure!(
            c.key().account == account.account && writes.insert(c.key()),
            "duplicate or cross-account operation write"
        );
        let Some(source) = reads.iter().find(|r| r.key() == c.key()) else {
            return Ok(ServiceDecision::Incomplete(format!(
                "source read missing for {:?}",
                c.key()
            )));
        };
        if !c.expected().matches(source) {
            return Ok(ServiceDecision::Stale(source.clone()));
        }
        conditions.push(c.expected().clone());
        for guard in c.dependencies() {
            let Some(r) = reads.iter().find(|r| r.key() == guard.key()) else {
                return Ok(ServiceDecision::Incomplete(format!(
                    "source guard missing for {:?}",
                    guard.key()
                )));
            };
            if !guard.matches(r) {
                return Ok(ServiceDecision::Stale(r.clone()));
            }
            conditions.push(guard.clone());
        }
    }
    let mut prospective = reads.to_vec();
    let mut next_account = account.clone();
    let mut accepted = Vec::new();
    let mut journals = Vec::new();
    let mut external_guards = Vec::new();
    while !changes.is_empty() {
        let ready = changes.iter().position(|c| {
            let mut required = crate::acceptance::required_references(c.key(), c.state());
            if let RecordState::Live(RecordValue::Service(s)) = c.state() {
                required.extend(super::dependency::required_keys(account.account, s));
                if let Some(qso) = source_qso(&s.payload) {
                    required.push(RecordKey {
                        account: account.account,
                        id: RecordId::Qso(qso),
                    });
                }
                required.extend(s.support.required.iter().map(|r| r.reference().key));
            }
            required
                .iter()
                .all(|key| !changes.iter().any(|other| other.key() == *key))
        });
        let Some(index) = ready else {
            anyhow::bail!("cyclic service operation dependencies")
        };
        let mut c = changes.remove(index);
        let current = prospective
            .iter()
            .find(|r| r.key() == c.key())
            .ok_or_else(|| anyhow::anyhow!("operation source observation disappeared"))?
            .clone();
        if matches!(c, NativeChange::Server(_))
            && let Observation::Accepted(old) = &current
            && old.state == *c.state()
        {
            continue;
        }
        let mut refs = crate::acceptance::required_references(c.key(), c.state());
        if let NativeChange::Server(change) = &mut c {
            if let RecordState::Live(RecordValue::Service(s)) = &mut change.state {
                refs.extend(super::dependency::required_keys(account.account, s));
                refs.extend(s.support.required.iter().map(|r| r.reference().key));
                if let ServicePayload::Association(a) = &s.payload {
                    refs.extend(prospective.iter().filter_map(|r|if let Observation::Accepted(row)=r && let Some(source)=super::payload(row) && source.support.operation==Some(id) && matches!(&source.payload,ServicePayload::Result(result) if result.external.as_ref()==Some(&a.external)){Some(row.key)}else{None}));
                }
                for key in &refs {
                    if let Some(Observation::Accepted(row)) =
                        prospective.iter().find(|r| r.key() == *key)
                    {
                        let req = if matches!(
                            key.id,
                            RecordId::Service(
                                ServiceRecordId::Attempt(_)
                                    | ServiceRecordId::Observation(_)
                                    | ServiceRecordId::Result(_)
                                    | ServiceRecordId::Assertion(_)
                            )
                        ) {
                            SupportRequirement::ExactFact(VersionReference::from(row))
                        } else {
                            SupportRequirement::CoveredEffect(VersionReference::from(row))
                        };
                        if !s.support.required.iter().any(|r| r.reference().key == *key) {
                            s.support.required.push(req);
                        }
                    }
                }
                if let ServicePayload::Disposition(d) = &s.payload
                    && let Some(aid) = d.association
                    && let Some(a) = prospective
                        .iter()
                        .filter_map(|r| {
                            if let Observation::Accepted(v) = r {
                                super::payload(v)
                            } else {
                                None
                            }
                        })
                        .find_map(|r| {
                            if let ServicePayload::Association(a) = &r.payload
                                && a.id == aid
                            {
                                Some(a)
                            } else {
                                None
                            }
                        })
                {
                    let qkey = RecordKey {
                        account: account.account,
                        id: RecordId::Qso(a.qso),
                    };
                    if let Some(Observation::Accepted(q)) =
                        prospective.iter().find(|r| r.key() == qkey)
                    {
                        s.support
                            .required
                            .push(SupportRequirement::CoveredEffect(VersionReference::from(q)));
                        refs.push(qkey);
                    }
                }
            }
            if let RecordState::Live(RecordValue::Service(s)) = &change.state
                && let Some(qso) = source_qso(&s.payload)
            {
                refs.push(RecordKey {
                    account: account.account,
                    id: RecordId::Qso(qso),
                });
            }
            for key in refs.iter().copied() {
                if let Some(r) = prospective.iter().find(|r| r.key() == key) {
                    let guard = VersionCondition::from_observation(r)?;
                    change.dependencies.retain(|d| d.key() != key);
                    change.dependencies.push(guard);
                }
            }
        }
        for key in refs {
            if !writes.contains(&key)
                && let Some(r) = reads.iter().find(|r| r.key() == key)
            {
                conditions.push(VersionCondition::from_observation(r)?);
            }
        }
        let registry = super::DestinationRegistryObservation {
            account: account.account,
            head: next_account.head,
            destinations: destinations.to_vec(),
        };
        let input = AcceptanceInput {
            destination_registry: Some(&registry),
            account: &next_account,
            membership,
            current: &current,
            references: &prospective,
            bindings,
        };
        let decision = match &c {
            NativeChange::Server(c) => plan_server_acceptance(c, &input, evidence)?,
            NativeChange::Client(c) => plan_acceptance(c, &input, evidence)?,
        };
        match decision {
            AcceptanceDecision::Commit(commit) => {
                external_guards.extend(commit.external_guards);
                next_account = commit.next_account;
                prospective.retain(|r| r.key() != commit.accepted.key);
                prospective.push(Observation::Accepted(commit.accepted.clone()));
                accepted.push(commit.accepted);
                journals.push(commit.journal);
            }
            AcceptanceDecision::Covered(_) => {}
            AcceptanceDecision::Stale(r) => return Ok(ServiceDecision::Stale(r)),
            AcceptanceDecision::Rejected(r) => {
                return Ok(ServiceDecision::Review(format!(
                    "operation admission rejected: {r:?}"
                )));
            }
        }
    }
    if accepted.is_empty() {
        return Ok(ServiceDecision::Unchanged);
    }
    let unit = CoupledRequirement {
        id,
        members: accepted.iter().map(VersionReference::from).collect(),
    };
    Ok(ServiceDecision::Ready(CoupledCommit {
        expected_head: account.head,
        expected_membership_generation: membership.generation,
        conditions,
        external_guards,
        next_account,
        accepted,
        journals,
        unit,
        job_updates,
    }))
}

/// Identify a fresh source-contact guard without making frozen evidence depend on its old body.
fn source_qso(payload: &ServicePayload) -> Option<QsoId> {
    match payload {
        ServicePayload::Imported(a) => Some(a.qso),
        ServicePayload::Attempt(a) => Some(a.qso),
        _ => None,
    }
}
