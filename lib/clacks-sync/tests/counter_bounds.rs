use crate::support;

use clacks_sync::account::{AccountMetadata, EvidenceView, InstalledCheckpoint};
use clacks_sync::clock::{ClockActor, RecordVector};
use clacks_sync::lineage::{BoundaryKind, Coverage, Lineage, TimelineBoundary};
use clacks_sync::membership::MembershipSnapshot;
use clacks_sync::record::Position;
use clacks_sync::transfer::{WindowManifest, WindowMode, WindowProgress, plan_batch};
use support::id;

const MAX: u64 = 9_223_372_036_854_775_807;

#[test]
fn vector_counter_range_and_checked_exhaustion() -> anyhow::Result<()> {
    let actor = ClockActor::Client(id(9));
    assert!(RecordVector::from_entries([(actor, MAX + 1)]).is_err());
    let vector = RecordVector::from_entries([(actor, MAX)])?;
    assert_eq!(vector.component(actor), MAX);
    assert!(vector.increment(actor, MAX).is_err());
    assert!(RecordVector::default().increment(actor, MAX + 1).is_err());
    assert!(
        RecordVector::from_entries([(actor, 0)])?
            .entries()
            .next()
            .is_none()
    );
    Ok(())
}

#[test]
fn coverage_and_boundary_reject_oversized_revisions() -> anyhow::Result<()> {
    assert!(Coverage::from_entries([(id(2), MAX + 1)]).is_err());
    let mut lineage = Lineage::new(id(1), id(2));
    assert!(
        lineage
            .add_boundary(TimelineBoundary {
                at: Position {
                    timeline: id(2),
                    revision: MAX + 1
                },
                includes: Coverage::default(),
                kind: BoundaryKind::RecoveredCoverage,
            })
            .is_err()
    );
    Ok(())
}

#[test]
fn public_membership_and_baseline_counters_are_validated() -> anyhow::Result<()> {
    let (mut account, mut baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.membership_generation = MAX + 1;
    let membership = MembershipSnapshot {
        account: id(1),
        generation: MAX + 1,
        clients: vec![],
    };
    assert!(membership.validate(&account).is_err());
    baseline.membership_generation = MAX + 1;
    let lineage = Lineage::new(id(1), id(2));
    let baselines = [baseline];
    let evidence = EvidenceView {
        lineage: &lineage,
        baselines: &baselines,
    };
    assert!(evidence.baseline(id(1), id(3)).is_err());
    Ok(())
}

#[test]
fn window_progress_cannot_advance_beyond_signed_maximum() -> anyhow::Result<()> {
    let (account, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let checkpoint = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: account.installed,
    };
    let manifest = WindowManifest::new(
        id(1),
        checkpoint.clone(),
        WindowMode::Full,
        vec![],
        vec![],
        vec![],
        vec![],
    )?;
    let baselines = [baseline];
    let evidence = EvidenceView {
        lineage: &baselines.first().unwrap().lineage,
        baselines: &baselines,
    };
    for generation in [MAX, MAX + 1] {
        let progress = WindowProgress {
            source: checkpoint.clone(),
            mode: WindowMode::Full,
            generation,
            installed: Default::default(),
            staged: vec![],
            completed_units: Default::default(),
        };
        assert!(plan_batch(&manifest, &progress, &[], &[], &evidence).is_err());
    }
    Ok(())
}

#[test]
fn infallibly_wrapped_coverage_is_checked_at_transition_boundaries() -> anyhow::Result<()> {
    let (mut account, mut baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let oversized = Coverage::from(Position {
        timeline: id(2),
        revision: MAX + 1,
    });
    account.installed = oversized.clone();
    let membership = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![],
    };
    assert!(membership.validate(&account).is_err());
    baseline.coverage = oversized;
    let lineage = Lineage::new(id(1), id(2));
    let baselines = [baseline];
    assert!(
        EvidenceView {
            lineage: &lineage,
            baselines: &baselines
        }
        .baseline(id(1), id(3))
        .is_err()
    );
    Ok(())
}

#[test]
fn public_high_water_is_checked_before_unchanged_preparation() -> anyhow::Result<()> {
    use clacks_sync::{
        client::{ClientRecord, LocalCounter, prepare_change},
        error::UnknownReason,
        record::{Observation, RecordId, RecordKey},
    };
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let baselines = [baseline];
    let evidence = EvidenceView {
        lineage: &baselines.first().unwrap().lineage,
        baselines: &baselines,
    };
    let key = RecordKey {
        account: id(1),
        id: RecordId::Qso(id(10)),
    };
    let current = ClientRecord {
        key,
        latest: Observation::Unavailable {
            key,
            reason: UnknownReason::MissingOriginal,
        },
        pending: None,
        retained: vec![],
    };
    assert!(
        prepare_change(
            &current,
            id(9),
            &LocalCounter {
                actor: id(9),
                high_water: MAX + 1
            },
            &[],
            &evidence
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn revision_and_membership_exhaustion_preserve_input() -> anyhow::Result<()> {
    use clacks_sync::membership::{ClientMembership, revoke};
    let (mut account, _) = AccountMetadata::initialize(id(1), id(2), id(3));
    let mut membership = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![ClientMembership {
            actor: id(9),
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    account.head.revision = MAX;
    assert!(revoke(id(9), &account, &membership).is_err());
    assert_eq!(account.head.revision, MAX);
    account.head.revision = 0;
    account.membership_generation = MAX;
    membership.generation = MAX;
    assert!(revoke(id(9), &account, &membership).is_err());
    assert!(membership.clients.first().unwrap().closed_at.is_none());
    Ok(())
}

#[test]
fn zero_origin_is_valid_but_zero_acceptance_is_not() -> anyhow::Result<()> {
    let (account, _) = AccountMetadata::initialize(id(1), id(2), id(3));
    account.validate()?;
    let mut accepted = support::accepted(1, "original");
    accepted.stamp.accepted_at.revision = 0;
    assert!(accepted.stamp.validate().is_err());
    assert!(accepted.validate().is_err());
    Ok(())
}

#[test]
fn service_support_checks_acquisition_context_before_use() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::ServicePayload,
    };
    use clacks_sync::{
        record::VersionStamp,
        service::{ServiceRecord, ServiceSupport},
    };
    let record = ServiceRecord {
        payload: ServicePayload::Destination(Destination {
            id: id(20),
            collection: RemoteCollection::Lotw {
                account: "public account".into(),
            },
            description: "description".into(),
        }),
        support: ServiceSupport {
            acquisition: Some(VersionStamp {
                accepted_at: Position {
                    timeline: id(2),
                    revision: MAX + 1,
                },
                baseline: id(3),
                vector: RecordVector::default(),
            }),
            ..Default::default()
        },
    };
    assert!(record.validate(id(1)).is_err());
    Ok(())
}

#[test]
fn window_boundary_coordinates_are_validated_without_installed_ancestry() -> anyhow::Result<()> {
    let (account, _) = AccountMetadata::initialize(id(1), id(2), id(3));
    let checkpoint = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: account.installed,
    };
    let invalid = TimelineBoundary {
        at: Position {
            timeline: id(4),
            revision: MAX + 1,
        },
        includes: Coverage::default(),
        kind: BoundaryKind::RecoveredCoverage,
    };
    assert!(
        WindowManifest::new(
            id(1),
            checkpoint,
            WindowMode::Full,
            vec![],
            vec![invalid],
            vec![],
            vec![]
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn accepted_context_descriptors_require_the_actual_server_component() -> anyhow::Result<()> {
    let mut record = support::accepted(1, "original");
    record.stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(2)), 2)])?;
    assert!(record.stamp.validate().is_err());
    Ok(())
}
