mod support;

#[test]
fn destination_control_identity_is_stable_and_streams_remain_separate() -> anyhow::Result<()> {
    use clacks_model::service::evidence::download::DownloadControl;
    let a = DownloadControl::new(support::id(42))?;
    let b = DownloadControl::new(support::id(42))?;
    let c = DownloadControl::new(support::id(43))?;
    assert_eq!(a.id, b.id);
    assert_ne!(a.id, c.id);
    assert!(a.active.is_none());
    assert!(a.progress.is_empty());
    Ok(())
}

#[test]
fn starts_compete_on_destination_control_guard() -> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{Destination, RemoteCollection},
            evidence::{
                ServicePayload,
                download::DownloadControl,
                observation::{QueryIdentity, QueryStream},
            },
        },
    };
    use clacks_sync::{
        account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
        membership::MembershipSnapshot,
        record::{Observation, RecordId, RecordKey},
        service::{
            download::{DownloadContext, DownloadStartInput, plan_start},
            operation::ServiceDecision,
        },
    };
    let (account, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let evidence = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let control = DownloadControl::new(support::id(42))?;
    let checkpoint = InstalledCheckpoint {
        account: account.account,
        baseline: b.id,
        coverage: account.installed.clone(),
    };
    let absent = |payload: ServicePayload| Observation::Absent {
        key: RecordKey {
            account: account.account,
            id: RecordId::Service(payload.id()),
        },
        checkpoint: checkpoint.clone(),
    };
    let query = QueryIdentity {
        destination: support::id(42),
        stream: QueryStream::Qrz,
        selected_fields: Default::default(),
        own_call: None,
        since: None,
    };
    let reads = [
        absent(ServicePayload::Download(control.clone())),
        Observation::Absent {
            key: RecordKey {
                account: account.account,
                id: RecordId::Service(
                    clacks_model::service::evidence::ServiceRecordId::Acquisition(support::id(100)),
                ),
            },
            checkpoint,
        },
    ];
    let destinations = [Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: Some(7) },
        description: "book".into(),
    }];
    let context = DownloadContext {
        id: support::id(101),
        account: &account,
        membership: &membership,
        reads: &reads,
        bindings: &[],
        destinations: &destinations,
        evidence: &evidence,
    };
    let input = DownloadStartInput {
        context: &context,
        control: &control,
        id: support::id(100),
        query: &query,
        at: UtcTimestamp::new(0, 0)?,
    };
    let ServiceDecision::Ready(a) = plan_start(&input)? else {
        panic!()
    };
    let ServiceDecision::Ready(b) = plan_start(&input)? else {
        panic!()
    };
    assert_eq!(a.conditions, b.conditions);
    assert_eq!(a.accepted.len(), 2);
    Ok(())
}

struct ReadFixture {
    account: clacks_sync::account::AccountMetadata,
    baseline: clacks_sync::account::Baseline,
    rows: Vec<clacks_sync::record::AcceptedRecord>,
    serial: u128,
}

impl ReadFixture {
    fn new() -> Self {
        let (account, baseline) = clacks_sync::account::AccountMetadata::initialize(
            support::id(1),
            support::id(2),
            support::id(3),
        );
        Self {
            account,
            baseline,
            rows: vec![],
            serial: 1000,
        }
    }
    fn next<T: std::str::FromStr>(&mut self) -> T
    where
        T::Err: std::fmt::Debug,
    {
        self.serial += 1;
        support::id(self.serial)
    }
    fn control(&self) -> clacks_model::service::evidence::download::DownloadControl {
        self.rows
            .iter()
            .find_map(|r| {
                if let clacks_sync::record::RecordState::Live(
                    clacks_sync::record::RecordValue::Service(s),
                ) = &r.state
                    && let clacks_model::service::evidence::ServicePayload::Download(c) = &s.payload
                {
                    Some(c.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                clacks_model::service::evidence::download::DownloadControl::new(support::id(42))
                    .unwrap()
            })
    }
    fn reads(
        &self,
        extra: &[clacks_model::service::evidence::ServiceRecordId],
    ) -> Vec<clacks_sync::record::Observation> {
        use clacks_sync::{
            account::InstalledCheckpoint,
            record::{Observation, RecordId, RecordKey},
        };
        let mut out: Vec<_> = self
            .rows
            .iter()
            .cloned()
            .map(Observation::Accepted)
            .collect();
        for id in extra {
            let key = RecordKey {
                account: self.account.account,
                id: RecordId::Service(*id),
            };
            if !out.iter().any(|r| r.key() == key) {
                out.push(Observation::Absent {
                    key,
                    checkpoint: InstalledCheckpoint {
                        account: self.account.account,
                        baseline: self.account.baseline,
                        coverage: self.account.installed.clone(),
                    },
                });
            }
        }
        out
    }
    fn install(&mut self, c: clacks_sync::service::operation::CoupledCommit) -> anyhow::Result<()> {
        assert_eq!(c.expected_head, self.account.head);
        self.account = c.next_account;
        for row in c.accepted {
            self.rows.retain(|r| r.key != row.key);
            self.rows.push(row);
        }
        self.rows = serde_json::from_str(&serde_json::to_string(&self.rows)?)?;
        Ok(())
    }
    fn start(
        &mut self,
    ) -> anyhow::Result<clacks_model::service::evidence::download::AcquisitionState> {
        use clacks_model::{
            scalar::UtcTimestamp,
            service::{
                destination::{Destination, RemoteCollection},
                evidence::{
                    ServicePayload, ServiceRecordId,
                    observation::{QueryIdentity, QueryStream},
                },
            },
        };
        use clacks_sync::{
            account::EvidenceView,
            membership::MembershipSnapshot,
            service::{download::*, operation::ServiceDecision},
        };
        let control = self.control();
        let id = self.next();
        let op = self.next();
        let reads = self.reads(&[
            ServiceRecordId::Download(control.id),
            ServiceRecordId::Acquisition(id),
        ]);
        let bases = [self.baseline.clone()];
        let evidence = EvidenceView {
            lineage: &self.baseline.lineage,
            baselines: &bases,
        };
        let membership = MembershipSnapshot {
            account: self.account.account,
            generation: 0,
            clients: vec![],
        };
        let destinations = [Destination {
            id: control.destination,
            collection: RemoteCollection::Qrz { book_id: Some(7) },
            description: "book".into(),
        }];
        let context = DownloadContext {
            id: op,
            account: &self.account,
            membership: &membership,
            reads: &reads,
            bindings: &[],
            destinations: &destinations,
            evidence: &evidence,
        };
        let query = QueryIdentity {
            destination: control.destination,
            stream: QueryStream::Qrz,
            selected_fields: Default::default(),
            own_call: None,
            since: None,
        };
        let ServiceDecision::Ready(c) = plan_start(&DownloadStartInput {
            context: &context,
            control: &control,
            id,
            query: &query,
            at: UtcTimestamp::new(0, 0)?,
        })?
        else {
            panic!()
        };
        let a = c
            .accepted
            .iter()
            .find_map(|r| {
                if let clacks_sync::record::RecordState::Live(
                    clacks_sync::record::RecordValue::Service(s),
                ) = &r.state
                    && let ServicePayload::Acquisition(a) = &s.payload
                {
                    Some(a.clone())
                } else {
                    None
                }
            })
            .unwrap();
        self.install(c)?;
        Ok(a)
    }
    fn observe(
        &mut self,
        a: &clacks_model::service::evidence::download::AcquisitionState,
        value: &str,
        ordering: (Option<i64>, Option<i64>),
    ) -> anyhow::Result<
        clacks_sync::service::operation::ServiceDecision<
            clacks_sync::service::operation::CoupledCommit,
        >,
    > {
        self.observe_report(
            a,
            value,
            ordering,
            clacks_model::service::evidence::observation::ConfirmationStatus::Confirmed,
        )
    }
    fn observe_report(
        &mut self,
        a: &clacks_model::service::evidence::download::AcquisitionState,
        value: &str,
        ordering: (Option<i64>, Option<i64>),
        confirmation: clacks_model::service::evidence::observation::ConfirmationStatus,
    ) -> anyhow::Result<
        clacks_sync::service::operation::ServiceDecision<
            clacks_sync::service::operation::CoupledCommit,
        >,
    > {
        self.observe_fields(a, value, ordering, confirmation, None)
    }

    fn observe_fields(
        &mut self,
        a: &clacks_model::service::evidence::download::AcquisitionState,
        value: &str,
        ordering: (Option<i64>, Option<i64>),
        confirmation: clacks_model::service::evidence::observation::ConfirmationStatus,
        dxcc: Option<&str>,
    ) -> anyhow::Result<
        clacks_sync::service::operation::ServiceDecision<
            clacks_sync::service::operation::CoupledCommit,
        >,
    > {
        use clacks_model::{
            scalar::{UtcSecond, UtcTimestamp},
            service::{
                destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
                evidence::{ServiceRecordId, observation::*},
            },
        };
        use clacks_sync::{
            account::EvidenceView, membership::MembershipSnapshot, service::download::*,
        };
        let oid = self.next();
        let disposition = self.next();
        let op = self.next();
        let reads = self.reads(&[
            ServiceRecordId::Observation(oid),
            ServiceRecordId::Disposition(disposition),
        ]);
        let bases = [self.baseline.clone()];
        let evidence = EvidenceView {
            lineage: &self.baseline.lineage,
            baselines: &bases,
        };
        let membership = MembershipSnapshot {
            account: self.account.account,
            generation: 0,
            clients: vec![],
        };
        let destinations = [Destination {
            id: a.query.destination,
            collection: RemoteCollection::Qrz { book_id: Some(7) },
            description: "book".into(),
        }];
        let context = DownloadContext {
            id: op,
            account: &self.account,
            membership: &membership,
            reads: &reads,
            bindings: &[],
            destinations: &destinations,
            evidence: &evidence,
        };
        let observation = RemoteObservation {
            id: oid,
            destination: a.query.destination,
            identity: RemoteIdentity::Complete(ExternalRecordKey {
                destination: a.query.destination,
                record: RemoteRecordId::Qrz(7),
            }),
            facts: RemoteFacts {
                own_station: StationReport {
                    name: Reported::Value(value.into()),
                    dxcc: dxcc
                        .map(|value| Reported::Value(value.into()))
                        .unwrap_or_default(),
                    ..Default::default()
                },
                contact: ContactReport {
                    confirmation: Reported::Value(confirmation),
                    ..Default::default()
                },
                ..Default::default()
            },
            ordering: ProviderOrdering {
                qso: ordering.0.map(UtcSecond::from_unix_seconds),
                confirmation: ordering.1.map(UtcSecond::from_unix_seconds),
            },
            acquisition: AcquisitionProvenance {
                id: a.id,
                query: a.query.clone(),
                observed_at: UtcTimestamp::new(0, 0)?,
            },
        };
        plan_observation(&ObservationInput {
            context: &context,
            acquisition: a,
            observation: &observation,
            disposition,
        })
    }
    fn close(
        &mut self,
        a: &clacks_model::service::evidence::download::AcquisitionState,
        complete: bool,
    ) -> anyhow::Result<()> {
        use clacks_model::{
            scalar::UtcTimestamp,
            service::{
                destination::{Destination, RemoteCollection},
                evidence::{ServicePayload, observation::RemoteIdentity},
            },
        };
        use clacks_sync::{
            account::EvidenceView,
            membership::MembershipSnapshot,
            service::{download::*, operation::ServiceDecision},
        };
        let control = self.control();
        let op = self.next();
        let reads = self.reads(&[]);
        let bases = [self.baseline.clone()];
        let evidence = EvidenceView {
            lineage: &self.baseline.lineage,
            baselines: &bases,
        };
        let membership = MembershipSnapshot {
            account: self.account.account,
            generation: 0,
            clients: vec![],
        };
        let destinations = [Destination {
            id: control.destination,
            collection: RemoteCollection::Qrz { book_id: Some(7) },
            description: "book".into(),
        }];
        let context = DownloadContext {
            id: op,
            account: &self.account,
            membership: &membership,
            reads: &reads,
            bindings: &[],
            destinations: &destinations,
            evidence: &evidence,
        };
        let ids: Vec<_> = self
            .rows
            .iter()
            .filter_map(|r| {
                if let clacks_sync::record::RecordState::Live(
                    clacks_sync::record::RecordValue::Service(s),
                ) = &r.state
                    && let ServicePayload::Observation(o) = &s.payload
                    && matches!(o.identity, RemoteIdentity::Complete(_))
                {
                    Some(o.id)
                } else {
                    None
                }
            })
            .collect();
        let ServiceDecision::Ready(c) = plan_progress(&DownloadProgressInput {
            context: &context,
            control: &control,
            acquisition: a,
            retained_inputs: &ids,
            response_complete: complete,
            failure: None,
            watermark: None,
            at: UtcTimestamp::new(1, 0)?,
        })?
        else {
            panic!()
        };
        self.install(c)
    }
}

#[test]
fn semantic_repeat_and_return_after_change_survive_reload() -> anyhow::Result<()> {
    use clacks_sync::service::operation::ServiceDecision;
    let mut f = ReadFixture::new();
    let a = f.start()?;
    let ServiceDecision::Ready(c) = f.observe(&a, "A", (None, None))? else {
        panic!()
    };
    f.install(c)?;
    assert!(matches!(
        f.observe(&a, "A", (None, None))?,
        ServiceDecision::Unchanged
    ));
    f.close(&a, true)?;
    let b = f.start()?;
    let ServiceDecision::Ready(c) = f.observe(&b, "B", (None, None))? else {
        panic!()
    };
    f.install(c)?;
    f.close(&b, true)?;
    let last = f.start()?;
    let ServiceDecision::Ready(c) = f.observe(&last, "A", (None, None))? else {
        panic!()
    };
    f.install(c)?;
    assert_eq!(
        f.rows
            .iter()
            .filter(|r| matches!(
                r.key.id,
                clacks_sync::record::RecordId::Service(
                    clacks_model::service::evidence::ServiceRecordId::Observation(_)
                )
            ))
            .count(),
        3
    );
    Ok(())
}

#[test]
fn failed_acquisition_releases_control_without_completed_progress() -> anyhow::Result<()> {
    let mut f = ReadFixture::new();
    let a = f.start()?;
    let captured = support::telemetry::capture(|| f.close(&a, false));
    assert!(captured.result.is_ok());
    assert_eq!(
        captured
            .event("download.closure.planned")
            .fields
            .get("acquisition_outcome")
            .map(String::as_str),
        Some("failed")
    );

    captured.result?;
    assert!(f.control().active.is_none());
    assert!(f.control().progress.is_empty());
    let next = f.start()?;
    assert_ne!(a.id, next.id);
    Ok(())
}

fn established(
    f: &ReadFixture,
) -> anyhow::Result<clacks_sync::service::combination::SelectedObservations> {
    use clacks_model::service::destination::{ExternalRecordKey, RemoteRecordId};
    use clacks_sync::{
        account::EvidenceView,
        service::{
            ServiceView,
            combination::{ObservationSelectionInput, select_observations},
        },
    };
    let bases = [f.baseline.clone()];
    let evidence = EvidenceView {
        lineage: &f.baseline.lineage,
        baselines: &bases,
    };
    let view = ServiceView {
        account: f.account.account,
        records: &f.rows,
        bindings: &[],
        evidence: &evidence,
    };
    select_observations(&ObservationSelectionInput {
        external: &ExternalRecordKey {
            destination: support::id(42),
            record: RemoteRecordId::Qrz(7),
        },
        view: &view,
    })
}

#[test]
fn qso_and_confirmation_timestamps_select_fields_independently() -> anyhow::Result<()> {
    use clacks_model::service::evidence::observation::{ConfirmationStatus, Reported};
    use clacks_sync::service::operation::ServiceDecision;
    let mut f = ReadFixture::new();
    let a = f.start()?;
    let ServiceDecision::Ready(c) = f.observe(&a, "A", (Some(100), Some(300)))? else {
        panic!()
    };
    f.install(c)?;
    f.close(&a, true)?;
    let b = f.start()?;
    let ServiceDecision::Ready(c) = f.observe_report(
        &b,
        "B",
        (Some(200), Some(200)),
        ConfirmationStatus::Unconfirmed,
    )?
    else {
        panic!()
    };
    f.install(c)?;
    let current = established(&f)?;
    assert_eq!(
        current.current.facts.own_station.name,
        Reported::Value("B".into())
    );
    assert_eq!(
        current.current.facts.contact.confirmation,
        Reported::Value(ConfirmationStatus::Confirmed)
    );
    assert!(current.uncertain.is_empty());
    Ok(())
}

#[test]
fn late_response_uses_original_start_context_after_explicit_failure() -> anyhow::Result<()> {
    use clacks_model::service::evidence::observation::Reported;
    use clacks_sync::service::operation::ServiceDecision;
    let mut f = ReadFixture::new();
    let old = f.start()?;
    f.close(&old, false)?;
    let new = f.start()?;
    let ServiceDecision::Ready(c) = f.observe(&new, "new", (None, None))? else {
        panic!()
    };
    f.install(c)?;
    let ServiceDecision::Ready(c) = f.observe(&old, "old", (None, None))? else {
        panic!("late response must retain its original evidence")
    };
    f.install(c)?;
    assert_eq!(
        established(&f)?.current.facts.own_station.name,
        Reported::Value("new".into())
    );
    assert_eq!(f.control().active, Some(new.id));
    Ok(())
}

#[test]
fn contradictory_mixed_ordering_keeps_uncertainty_until_a_dominating_refresh() -> anyhow::Result<()>
{
    use clacks_model::service::evidence::observation::{ConfirmationStatus, Reported};
    use clacks_sync::service::operation::ServiceDecision;
    let mut f = ReadFixture::new();
    for (value, time, status) in [
        ("CANARY-A", Some(3), ConfirmationStatus::Confirmed),
        ("CANARY-B", None, ConfirmationStatus::Unconfirmed),
        ("CANARY-C", Some(1), ConfirmationStatus::Confirmed),
    ] {
        let a = f.start()?;
        let ServiceDecision::Ready(c) = f.observe_report(&a, value, (None, time), status)? else {
            panic!()
        };
        f.install(c)?;
        f.close(&a, true)?;
    }
    let captured = support::telemetry::capture(|| established(&f));
    assert!(!format!("{:?}", captured.observations).contains("CANARY"));
    let event = captured.event("observation.field_uncertain");
    assert_eq!(
        event.fields.get("field").map(String::as_str),
        Some("contact.confirmation")
    );
    assert_eq!(
        event
            .fields
            .get("conflicting_observation_count")
            .map(String::as_str),
        Some("3")
    );
    assert_eq!(event.parent, captured.span("sync.observations.select").id);
    let selected = captured.result?;
    assert_eq!(
        selected.current.facts.contact.confirmation,
        Reported::NotReported
    );
    assert!(
        selected
            .uncertain
            .iter()
            .any(|p| p == "contact.confirmation")
    );
    let a = f.start()?;
    let ServiceDecision::Ready(c) =
        f.observe_report(&a, "D", (None, Some(4)), ConfirmationStatus::Confirmed)?
    else {
        panic!()
    };
    f.install(c)?;
    let captured = support::telemetry::capture(|| established(&f));
    assert!(captured.observations.events.iter().all(|e| {
        e.fields
            .get("event")
            .is_none_or(|v| v != "observation.field_uncertain")
    }));
    let selected = captured.result?;
    assert_eq!(
        selected.current.facts.contact.confirmation,
        Reported::Value(ConfirmationStatus::Confirmed)
    );
    assert!(
        !selected
            .uncertain
            .iter()
            .any(|p| p == "contact.confirmation")
    );
    Ok(())
}

#[test]
fn ordering_only_refresh_preserves_the_current_versions_disposition() -> anyhow::Result<()> {
    use clacks_model::service::evidence::{ServicePayload, processing::ProcessingDecision};
    use clacks_sync::service::operation::ServiceDecision;
    let mut f = ReadFixture::new();
    let a = f.start()?;
    let ServiceDecision::Ready(c) = f.observe(&a, "A", (Some(10), None))? else {
        panic!()
    };
    let old = c.accepted.iter().find_map(|r| {
        if let Some(s) = service_payload(r) {
            if let ServicePayload::Disposition(d) = &s.payload {
                Some(d.clone())
            } else {
                None
            }
        } else {
            None
        }
    });
    f.install(c)?;
    let d = old.unwrap();
    f.reject(d)?;
    f.close(&a, true)?;
    for (value, timestamp, expected) in [
        ("A", 20, ProcessingDecision::Rejected),
        ("B", 30, ProcessingDecision::PendingReview),
        ("B", 40, ProcessingDecision::PendingReview),
        ("A", 50, ProcessingDecision::PendingReview),
        ("A", 60, ProcessingDecision::PendingReview),
    ] {
        let acquisition = f.start()?;
        let ServiceDecision::Ready(commit) =
            f.observe(&acquisition, value, (Some(timestamp), None))?
        else {
            panic!("missing occurrence");
        };
        let decision = commit
            .accepted
            .iter()
            .filter_map(service_payload)
            .find_map(|s| {
                if let ServicePayload::Disposition(d) = &s.payload {
                    Some(d.decision)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(
            decision, expected,
            "wrong disposition for {value} at {timestamp}"
        );
        f.install(commit)?;
        f.close(&acquisition, true)?;
    }
    Ok(())
}

fn service_payload(
    r: &clacks_sync::record::AcceptedRecord,
) -> Option<&clacks_sync::service::ServiceRecord> {
    if let clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Service(s)) =
        &r.state
    {
        Some(s)
    } else {
        None
    }
}

impl ReadFixture {
    fn reject(
        &mut self,
        d: clacks_model::service::evidence::processing::ProcessingDisposition,
    ) -> anyhow::Result<()> {
        use clacks_model::service::{
            destination::{Destination, RemoteCollection},
            evidence::processing::{ProcessingDecision, ProcessingDisposition},
        };
        use clacks_sync::{
            account::EvidenceView,
            membership::MembershipSnapshot,
            service::operation::{
                ServiceDecision, ServiceOperation, ServiceOperationInput, plan_operation,
            },
        };
        let rejected = ProcessingDisposition {
            decision: ProcessingDecision::Rejected,
            ..d.clone()
        };
        let op = ServiceOperation::CompleteObservation {
            observation: d.observation,
            disposition: rejected,
            association: None,
            qso_change: None,
            selected: None,
        };
        let reads = self.reads(&[]);
        let bases = [self.baseline.clone()];
        let ev = EvidenceView {
            lineage: &self.baseline.lineage,
            baselines: &bases,
        };
        let m = MembershipSnapshot {
            account: self.account.account,
            generation: 0,
            clients: vec![],
        };
        let ds = [Destination {
            id: support::id(42),
            collection: RemoteCollection::Qrz { book_id: Some(7) },
            description: "book".into(),
        }];
        let ServiceDecision::Ready(c) = plan_operation(&ServiceOperationInput {
            id: support::id(9999),
            account: &self.account,
            membership: &m,
            reads: &reads,
            bindings: &[],
            destinations: &ds,
            evidence: &ev,
            operation: &op,
            matching: None,
        })?
        else {
            panic!()
        };
        self.install(c)?;
        Ok(())
    }
}

#[test]
fn ordering_only_partial_refresh_keeps_its_rejection_and_omitted_facts() -> anyhow::Result<()> {
    use clacks_model::service::evidence::{
        ServicePayload,
        observation::{ConfirmationStatus, Reported},
        processing::ProcessingDecision,
    };
    use clacks_sync::service::operation::ServiceDecision;
    let mut f = ReadFixture::new();
    let a = f.start()?;
    let ServiceDecision::Ready(commit) = f.observe_fields(
        &a,
        "A",
        (Some(10), None),
        ConfirmationStatus::Confirmed,
        Some("291"),
    )?
    else {
        panic!()
    };
    f.install(commit)?;
    f.close(&a, true)?;
    let b = f.start()?;
    let ServiceDecision::Ready(commit) = f.observe(&b, "B", (Some(20), None))? else {
        panic!()
    };
    let disposition = commit
        .accepted
        .iter()
        .filter_map(service_payload)
        .find_map(|s| match &s.payload {
            ServicePayload::Disposition(d) => Some(d.clone()),
            _ => None,
        })
        .unwrap();
    f.install(commit)?;
    f.reject(disposition)?;
    f.close(&b, true)?;
    let c = f.start()?;
    let ServiceDecision::Ready(commit) = f.observe(&c, "B", (Some(30), None))? else {
        panic!()
    };
    assert!(commit.accepted.iter().filter_map(service_payload).any(|s| matches!(&s.payload, ServicePayload::Disposition(d) if d.decision == ProcessingDecision::Rejected)));
    f.install(commit)?;
    assert_eq!(
        established(&f)?.current.facts.own_station.dxcc,
        Reported::Value("291".into())
    );
    Ok(())
}
