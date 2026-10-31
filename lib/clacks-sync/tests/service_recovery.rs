use crate::support;

#[test]
fn recovered_same_id_mutation_is_checked_before_covered_shortcut() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{ServicePayload, ServiceRecordId, submission::*},
    };
    use clacks_sync::{
        acceptance::AcceptanceInput,
        account::{AccountMetadata, EvidenceView},
        membership::MembershipSnapshot,
        record::{Observation, RecordId, RecordState, RecordValue},
        recovery::plan_recovered_install,
        service::{ServiceRecord, ServiceSupport},
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let result = SubmissionResult {
        external_attempt: None,
        id: support::id(101),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(support::id(102)),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at: clacks_model::scalar::UtcTimestamp::new(0, 0)?,
        diagnostic: None,
        external: None,
    };
    let mut incoming = support::accepted(190, "fixture");
    support::edit_accepted(&mut incoming, |incoming| {
        incoming.key.id = RecordId::Service(ServiceRecordId::Result(result.id));

        incoming.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Result(result),
            support: ServiceSupport {
                destination: Some(destination),
                ..Default::default()
            },
        })));
    });
    let mut newer = incoming.clone();
    newer.stamp = support::accepted(200, "fixture").stamp;
    support::edit_accepted(&mut newer, |newer| {
        if let RecordState::Live(RecordValue::Service(s)) = &mut newer.state
            && let ServicePayload::Result(r) = &mut s.payload
        {
            r.outcome = SubmissionOutcome::Rejected("different".into());
        }
    });
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let current = Observation::Accepted(newer);
    assert!(
        plan_recovered_install(
            &incoming,
            None,
            support::id(9),
            &AcceptanceInput {
                destination_registry: None,
                account: &account,
                membership: &membership,
                current: &current,
                references: &[],
                bindings: &[]
            },
            &ev
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn covered_active_association_is_not_reinstalled_after_contact_deletion() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
        evidence::{
            ServicePayload,
            processing::{AssociationState, ExternalAssociation},
        },
    };
    use clacks_sync::{
        acceptance::AcceptanceInput,
        account::{AccountMetadata, EvidenceView},
        membership::{ClientMembership, MembershipSnapshot},
        record::{Observation, RecordId, RecordState, RecordValue},
        recovery::{RecoveredDecision, plan_recovered_install},
        service::{
            BindingVersion, DestinationRegistryObservation, ExternalBindingObservation,
            ServiceRecord, ServiceSupport,
        },
    };
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![ClientMembership {
            actor: support::id(4),
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let a = ExternalAssociation {
        id: support::id(101),
        external: ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz("7".parse().unwrap()),
        },
        qso: support::id(10),
        state: AssociationState::Active,
    };
    let mut old = support::accepted(190, "fixture");
    support::edit_accepted(&mut old, |old| {
        old.key.id = RecordId::Service(ServicePayload::Association(a.clone()).id());

        old.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Association(a.clone()),
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        })));
    });
    let mut new = old.clone();
    new.stamp = support::accepted(200, "fixture").stamp;
    support::edit_accepted(&mut new, |new| {
        if let RecordState::Live(RecordValue::Service(s)) = &mut new.state
            && let ServicePayload::Association(a) = &mut s.payload
        {
            a.state = AssociationState::DeletedQso;
        }
    });
    let mut deleted = support::accepted(200, "fixture");

    support::edit_accepted(&mut deleted, |deleted| {
        deleted.state = RecordState::Deleted;
    });
    let bindings = [ExternalBindingObservation {
        account: account.account,
        external: a.external.clone(),
        binding: Some(BindingVersion {
            association: a.id,
            qso: a.qso,
            state: AssociationState::DeletedQso,
            stamp: new.stamp.clone(),
        }),
    }];
    let registry = DestinationRegistryObservation {
        account: account.account,
        head: account.head,
        destinations: vec![destination],
    };
    let current = Observation::Accepted(new);
    let refs = [Observation::Accepted(deleted)];
    assert!(matches!(
        plan_recovered_install(
            &old,
            None,
            support::id(4),
            &AcceptanceInput {
                destination_registry: Some(&registry),
                account: &account,
                membership: &membership,
                current: &current,
                references: &refs,
                bindings: &bindings
            },
            &ev
        )?,
        RecoveredDecision::Covered
    ));
    Ok(())
}
