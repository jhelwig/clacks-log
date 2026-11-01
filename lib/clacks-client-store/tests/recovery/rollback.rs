use crate::support;
use clacks_client_store::{
    save,
    store::{ClientStore, StoreSettings},
};

use super::receive_interpretation;

#[tokio::test]
async fn rollback_ancestry_and_recovered_envelopes_preserve_local_tail_and_old_receipt()
-> anyhow::Result<()> {
    use clacks_client_store::read;
    use clacks_sync::{
        acceptance::AcceptanceInput,
        account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
        lineage::Coverage,
        membership::{ClientMembership, MembershipSnapshot},
        record::Observation,
        recovery::{
            HistoryCheck, HistoryObservation, RecoveredDecision, check_history,
            plan_recovered_install, plan_recovery_fork,
        },
        transfer::{RecordRequirement, WindowMode},
    };

    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;

    let (restored, baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let accepted = support::accepted(1, support::contact(support::qso(support::id(10))));
    let completed = InstalledCheckpoint {
        account: restored.account,
        baseline: baseline.id,
        coverage: Coverage::from(accepted.stamp.accepted_at()),
    };
    let requirement = RecordRequirement {
        key: accepted.key(),
        stamp: accepted.stamp.clone(),
        deleted: false,
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: completed.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![requirement.clone()],
            boundaries: vec![],
            baselines: vec![baseline.clone()],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };

    let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming
        .page(&store, std::slice::from_ref(&accepted))
        .await?;
    incoming.finish(&store).await?;

    let row = save::load(&store, accepted.key()).await?.unwrap();
    let mut qso = support::qso(support::id(10));
    support::edit(&mut qso, |qso| {
        qso.comment = Some("Keep local changes across server restoration".into());
        Ok(())
    });
    save::save(&store, accepted.key(), Some(&row), support::contact(qso)).await?;

    let membership = MembershipSnapshot {
        account: restored.account,
        generation: 0,
        clients: vec![ClientMembership {
            actor: support::id(9),
            created_at: restored.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    let bases = [baseline.clone()];
    let old = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &bases,
    };
    assert!(matches!(
        check_history(
            &HistoryObservation {
                account: restored.account,
                advertised: restored.head,
                remembered: completed.coverage.clone()
            },
            &old
        )?,
        HistoryCheck::Regression { .. }
    ));

    let fork = plan_recovery_fork(support::id(5), &restored, &membership, &restored.installed)?;
    let mut ancestry = baseline.lineage.clone();
    ancestry.add_boundary(fork.boundary.clone())?;

    receive_interpretation(&store, &completed, &[], &ancestry).await?;

    let retained = save::load(&store, accepted.key()).await?.unwrap();
    assert_eq!(retained.latest(), &Observation::Accepted(accepted.clone()));

    let evidence = EvidenceView {
        lineage: &ancestry,
        baselines: &bases,
    };
    let absence = Observation::Absent {
        key: accepted.key(),
        checkpoint: InstalledCheckpoint {
            account: restored.account,
            baseline: baseline.id,
            coverage: fork.next_account.installed.clone(),
        },
    };

    let RecoveredDecision::Install {
        envelope,
        next_account,
        ..
    } = plan_recovered_install(
        &accepted,
        None,
        support::id(9),
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &fork.next_account,
            membership: &membership,
            current: &absence,
            references: &[],
        },
        &evidence,
    )?
    else {
        panic!("authorized hub recovery")
    };

    assert_eq!(envelope, accepted);

    let source = InstalledCheckpoint {
        account: restored.account,
        baseline: baseline.id,
        coverage: next_account.installed,
    };
    let recovered = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![requirement],
            boundaries: vec![fork.boundary],
            baselines: bases.to_vec(),
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };

    let mut incoming = crate::incoming::Incoming::begin(&store, &recovered).await?;
    incoming.page(&store, &[envelope]).await?;
    let receipt = incoming.finish(&store).await?;

    assert!(receipt.coverage.covers(accepted.stamp.accepted_at()));
    assert_eq!(
        save::load(&store, accepted.key())
            .await?
            .unwrap()
            .working_state(),
        retained.working_state()
    );

    store.close().await?;

    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;

    let mut snapshot = read::snapshot(&store).await?;
    assert_eq!(snapshot.checkpoint().await?, Some(receipt));
    assert_eq!(
        snapshot.record(accepted.key()).await?.unwrap().latest(),
        &Observation::Accepted(accepted)
    );
    snapshot.close().await?;

    store.close().await?;

    Ok(())
}
