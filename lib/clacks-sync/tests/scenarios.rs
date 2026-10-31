use crate::support;
use clacks_sync::{
    acceptance::{AcceptanceDecision, AcceptanceInput, plan_acceptance},
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{
        ClientRecord, LocalCounter, OriginalBasis, PendingRecord, Preparation,
        preparation::prepare_change, receive_accepted,
    },
    lineage::Coverage,
    membership::{ClientMembership, MembershipSnapshot},
    record::{Observation, Position},
};
use support::{accepted, id};

fn members(ack: u64) -> MembershipSnapshot {
    MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![ClientMembership {
            actor: id(4),
            created_at: Position {
                timeline: id(2),
                revision: 0,
            },
            closed_at: None,
            acknowledged: Some(InstalledCheckpoint {
                account: id(1),
                baseline: id(3),
                coverage: Coverage::from_entries([(id(2), ack)]).unwrap(),
            }),
        }],
    }
}

#[test]
fn prepared_acceptance_lost_response_and_later_local_edit_compose() {
    let (mut account, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 200;
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let m = members(200);
    let a = accepted(200, "A");
    let c = clacks_sync::client::ClientRecordFields {
        retained: Vec::new(),
        key: a.key(),
        latest: Observation::Accepted(a.clone()),
        pending: Some(PendingRecord {
            witness: None,
            key: a.key(),
            original: OriginalBasis::Accepted(a.clone()),
            working: accepted(200, "B").state().clone(),
            prepared: None,
        }),
    }
    .try_into()
    .expect("coherent client fixture");
    let Preparation::Ready {
        mut record,
        change,
        counter,
    } = prepare_change(
        &c,
        id(4),
        &LocalCounter {
            actor: id(4),
            high_water: 0,
        },
        &[],
        &ev,
    )
    .unwrap()
    else {
        panic!("ready")
    };
    let old = Observation::Accepted(a);
    let AcceptanceDecision::Commit(commit) = plan_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &m,
            current: &old,
            references: &[],
        },
        &ev,
    )
    .unwrap() else {
        panic!("accepted")
    };
    support::edit_client(&mut record, |fields| {
        fields.pending.as_mut().unwrap().working = accepted(200, "C").state().clone();
    });
    let reloaded: ClientRecord =
        serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
    let update = receive_accepted(&reloaded, &commit.accepted, &ev).unwrap();
    assert!(update.review.is_none());
    let Preparation::Ready { change: later, .. } =
        prepare_change(&update.record, id(4), &counter, &[], &ev).unwrap()
    else {
        panic!("later change")
    };
    assert_eq!(
        later
            .vector()
            .component(clacks_sync::clock::ClockActor::Client(id(4))),
        2
    );
    assert!(matches!(
        plan_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &commit.next_account,
                membership: &m,
                current: &Observation::Accepted(commit.accepted),
                references: &[]
            },
            &ev
        )
        .unwrap(),
        AcceptanceDecision::Covered(_)
    ));
}
