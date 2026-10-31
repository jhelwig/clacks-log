use crate::support;

use clacks_sync::account::{AccountMetadata, EvidenceView};
use clacks_sync::clock::{ClockActor, RecordVector};
use clacks_sync::lineage::{BoundaryKind, Coverage, Lineage, TimelineBoundary};
use clacks_sync::membership::MembershipSnapshot;
use clacks_sync::record::Position;
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
        client::{LocalCounter, preparation::prepare_change},
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
    let current = clacks_sync::client::ClientRecordFields {
        key,
        latest: Observation::Unavailable {
            key,
            reason: UnknownReason::MissingOriginal,
        },
        pending: None,
        retained: vec![],
    }
    .try_into()
    .expect("coherent client fixture");
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
    assert!(
        clacks_sync::record::VersionStamp::new(account.head, id(3), RecordVector::default())
            .is_err()
    );
    Ok(())
}

#[test]
fn accepted_context_rejects_out_of_range_positions() {
    assert!(
        clacks_sync::record::VersionStamp::new(
            Position {
                timeline: id(2),
                revision: MAX + 1
            },
            id(3),
            RecordVector::default(),
        )
        .is_err()
    );
}

#[test]
fn window_boundary_coordinates_are_validated_without_installed_ancestry() -> anyhow::Result<()> {
    let invalid = TimelineBoundary {
        at: Position {
            timeline: id(4),
            revision: MAX + 1,
        },
        includes: Coverage::default(),
        kind: BoundaryKind::RecoveredCoverage,
    };
    let (account, _) = AccountMetadata::initialize(id(1), id(2), id(3));
    let receipt = clacks_sync::transfer::receipt::ReceiptState::begin(
        &clacks_sync::transfer::discovery::DiscoveryStart {
            source: clacks_sync::account::InstalledCheckpoint {
                account: account.account,
                baseline: account.baseline,
                coverage: account.installed,
            },
            mode: clacks_sync::transfer::WindowMode::Full,
        },
    )?;
    assert!(
        clacks_sync::transfer::receipt::register_page(
            &receipt,
            &clacks_sync::transfer::discovery::DiscoveryPage {
                sequence: 0,
                requirements: vec![],
                accepted: vec![],
                support: vec![],
                baselines: vec![],
                boundaries: vec![invalid],
                units: vec![],
            }
        )
        .is_err()
    );

    Ok(())
}

#[test]
fn accepted_context_descriptors_require_the_actual_server_component() -> anyhow::Result<()> {
    let record = support::accepted(1, "original");
    assert!(
        clacks_sync::record::VersionStamp::new(
            record.stamp.accepted_at(),
            record.stamp.baseline(),
            RecordVector::from_entries([(ClockActor::Server(id(2)), 2)])?,
        )
        .is_err()
    );
    Ok(())
}
