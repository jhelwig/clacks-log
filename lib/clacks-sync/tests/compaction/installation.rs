use crate::support::{accepted, id};
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    clock::{ClockActor, RecordVector},
    compaction::installation::plan_installation,
    compare::{RecordRelation, compare_records},
    lineage::Coverage,
    membership::{ClientMembership, MembershipSnapshot},
    record::{AcceptedRecord, Position, VersionStamp},
    transfer::RecordRequirement,
};

#[test]
fn installation_respects_authorized_acknowledgments() -> anyhow::Result<()> {
    let (mut account, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.head.revision = 200;
    account.installed = Coverage::from(account.head);
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };

    let mut membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: [(4, 100, true), (5, 150, false), (6, 180, false)]
            .into_iter()
            .map(|(actor, revision, closed)| ClientMembership {
                actor: id(actor),
                created_at: Position {
                    timeline: id(2),
                    revision: 0,
                },
                closed_at: closed.then_some(Position {
                    timeline: id(2),
                    revision,
                }),
                acknowledged: Some(InstalledCheckpoint {
                    account: account.account,
                    baseline: baseline.id,
                    coverage: Coverage::from(Position {
                        timeline: id(2),
                        revision,
                    }),
                }),
            })
            .collect(),
    };

    let record = accepted(200, "current");
    let stamp = VersionStamp::new(
        record.stamp.accepted_at(),
        baseline.id,
        RecordVector::from_entries([
            (ClockActor::Server(id(2)), 200),
            (ClockActor::Client(id(4)), 3),
            (ClockActor::Client(id(5)), 1),
        ])?,
    )?;
    let record = AcceptedRecord::new(record.key(), record.state().clone(), stamp)?;
    let descriptor = RecordRequirement {
        key: record.key(),
        stamp: record.stamp.clone(),
        deleted: false,
    };

    let installation = plan_installation(
        id(9),
        &account,
        &membership,
        std::slice::from_ref(&descriptor),
        &evidence,
    )?
    .ok_or_else(|| anyhow::anyhow!("closed component must be collectible"))?;
    assert_eq!(
        installation.baseline.coverage,
        Coverage::from(Position {
            timeline: id(2),
            revision: 150
        })
    );

    let converted = installation.conversions.first().unwrap();
    assert_eq!(
        converted
            .projected
            .stamp
            .vector()
            .component(ClockActor::Client(id(4))),
        0
    );
    assert_eq!(
        converted
            .projected
            .stamp
            .vector()
            .component(ClockActor::Client(id(5))),
        1
    );
    assert_eq!(
        converted.projected.stamp.accepted_at(),
        record.stamp.accepted_at()
    );

    let definitions = [baseline.clone(), installation.baseline.clone()];
    let projected = AcceptedRecord::new(
        record.key(),
        record.state().clone(),
        converted.projected.stamp.clone(),
    )?;
    assert_eq!(
        compare_records(
            &record,
            &projected,
            &EvidenceView {
                lineage: &baseline.lineage,
                baselines: &definitions
            }
        )?,
        RecordRelation::SameVersion
    );

    membership.clients.get_mut(1).unwrap().acknowledged = None;
    assert!(
        plan_installation(
            id(10),
            &account,
            &membership,
            std::slice::from_ref(&descriptor),
            &evidence
        )?
        .is_none()
    );
    membership
        .clients
        .retain(|member| member.closed_at.is_some());

    let isolated =
        plan_installation(id(11), &account, &membership, &[descriptor], &evidence)?.unwrap();
    assert_eq!(isolated.baseline.coverage, account.installed);
    assert!(
        plan_installation(
            id(12),
            &installation.account,
            &MembershipSnapshot {
                generation: 0,
                clients: vec![],
                account: account.account
            },
            std::slice::from_ref(&converted.projected),
            &EvidenceView {
                lineage: &baseline.lineage,
                baselines: &definitions
            }
        )?
        .is_none()
    );

    Ok(())
}
