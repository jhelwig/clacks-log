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
    f.close(&a, false)?;
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
        ("A", Some(3), ConfirmationStatus::Confirmed),
        ("B", None, ConfirmationStatus::Unconfirmed),
        ("C", Some(1), ConfirmationStatus::Confirmed),
    ] {
        let a = f.start()?;
        let ServiceDecision::Ready(c) = f.observe_report(&a, value, (None, time), status)? else {
            panic!()
        };
        f.install(c)?;
        f.close(&a, true)?;
    }
    let selected = established(&f)?;
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
    let selected = established(&f)?;
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
