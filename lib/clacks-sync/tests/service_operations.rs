use crate::support;
use clacks_model::service::{
    destination::{ExternalRecordKey, RemoteRecordId},
    evidence::processing::{AssociationState, ExternalAssociation},
};
use clacks_sync::service::ExternalBindingObservation;
use clacks_sync::{
    account::AccountMetadata,
    record::{Observation, RecordId},
};

fn remote(
    q: &clacks_model::contact::Qso,
) -> clacks_model::service::evidence::observation::RemoteObservation {
    use clacks_model::service::evidence::observation::*;
    RemoteObservation {
        id: support::id(501),
        destination: support::id(42),
        identity: RemoteIdentity::Complete(ExternalRecordKey {
            destination: support::id(42),
            record: RemoteRecordId::Qrz("7".parse().unwrap()),
        }),
        facts: RemoteFacts {
            contact: ContactReport {
                own_call: Reported::Value(q.own_station().callsign.clone()),
                worked_call: Reported::Value(q.contacted_station().callsign.clone()),
                start: Reported::Value(ObservedStart::Second(q.start())),
                band: Reported::Value("20m".into()),
                mode: Reported::Value(ModeReport {
                    concrete: "FT4".into(),
                    parent: Some("MFSK".into()),
                    group: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        },
        ordering: Default::default(),
        acquisition: AcquisitionProvenance {
            id: support::id(502),
            query: QueryIdentity {
                destination: support::id(42),
                stream: QueryStream::Qrz,
                selected_fields: Default::default(),
                own_call: None,
                since: None,
            },
            observed_at: clacks_model::scalar::UtcTimestamp::new(1000, 0).unwrap(),
        },
    }
}

#[test]
fn candidates_respect_identity_hierarchy_and_time_coverage() -> anyhow::Result<()> {
    use clacks_model::{
        recognition::RecognitionCatalog,
        scalar::{RecordedCallsign, UtcSecond},
        service::{
            evidence::observation::{ObservedStart, Reported},
            lotw::LotwConfiguration,
        },
    };
    use clacks_sync::service::association::{CandidateInput, classify_candidates};
    let row = support::accepted(200, "fixture");
    let RecordId::Qso(qid) = row.key().id else {
        panic!()
    };
    let q = support::qso(qid);
    let mut obs = remote(&q);
    let rows = [row];
    let cfg = LotwConfiguration::bundled()?;
    let classify = |obs: &_| {
        classify_candidates(&CandidateInput {
            observation: obs,
            account: support::id(1),
            account_qsos: &rows,
            recognition: RecognitionCatalog::adif_318().unwrap(),
            lotw_configuration: &cfg,
        })
    };
    assert_eq!(classify(&obs)?.confident, vec![qid]);
    for (offset, count) in [(1, 1), (1800, 1), (1801, 0), (-1800, 1), (-1801, 0)] {
        obs.facts.contact.start = Reported::Value(ObservedStart::Second(
            UtcSecond::from_unix_seconds(1000 + offset),
        ));
        let c = classify(&obs)?;
        assert!(c.confident.is_empty());
        assert_eq!(c.possible.len(), count);
    }
    obs.facts.contact.start = Reported::Value(ObservedStart::Minute(
        UtcSecond::from_unix_seconds(960).try_into().unwrap(),
    ));
    assert_eq!(classify(&obs)?.confident, vec![qid]);
    if let Reported::Value(m) = &mut obs.facts.contact.mode {
        m.concrete = "MFSK".into();
        m.parent = None;
    }
    assert_eq!(classify(&obs)?.possible.len(), 1);
    obs.facts.contact.own_call = Reported::NotReported;
    assert_eq!(classify(&obs)?.possible.len(), 1);
    obs.facts.contact.worked_call = Reported::Value(RecordedCallsign::new("K1ABC/P")?);
    assert!(classify(&obs)?.possible.is_empty());
    Ok(())
}

#[test]
fn generic_binding_admission_rejects_historical_reactivation() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{ServicePayload, ServiceRecordId},
    };
    use clacks_sync::{
        acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange, plan_server_acceptance},
        account::EvidenceView,
        client::VersionCondition,
        membership::MembershipSnapshot,
        record::{RecordKey, RecordState, RecordValue},
        service::{BindingVersion, ServiceRecord, ServiceSupport},
    };
    let q = support::accepted(200, "fixture");
    let RecordId::Qso(qso) = q.key().id else {
        panic!()
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let a = ExternalAssociation {
        id: support::id(11),
        external: ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz("7".parse().unwrap()),
        },
        qso,
        state: AssociationState::Superseded,
    };
    let mut old = q.clone();

    support::edit_accepted(&mut old, |old| {
        old.key = RecordKey {
            account: q.key().account,
            id: RecordId::Service(ServiceRecordId::Association(a.id)),
        };
        old.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Association(a.clone()),
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        })));
    });
    let current = Observation::Accepted(old.clone());
    let mut new = old.state().clone();
    if let RecordState::Live(RecordValue::Service(s)) = &mut new
        && let ServicePayload::Association(a) = &mut s.payload
    {
        a.state = AssociationState::Active;
    }
    let bindings = [ExternalBindingObservation {
        account: q.key().account,
        external: a.external.clone(),
        binding: Some(BindingVersion {
            association: a.id,
            qso,
            stamp: old.stamp.clone(),
            state: a.state,
        }),
    }];
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
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
    let change = ServerChange {
        key: old.key(),
        state: new,
        expected: VersionCondition::from_observation(&current)?,
        dependencies: vec![],
    };
    let registry = clacks_sync::service::DestinationRegistryObservation {
        account: account.account,
        head: account.head,
        destinations: vec![destination],
    };
    assert!(matches!(
        plan_server_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: Some(&registry),
                account: &account,
                membership: &membership,
                current: &current,
                references: &[Observation::Accepted(q)],
                bindings: &bindings
            },
            &evidence
        )?,
        AcceptanceDecision::Rejected(_)
    ));
    Ok(())
}
