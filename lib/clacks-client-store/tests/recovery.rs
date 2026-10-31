use crate::support;
use clacks_client_store::{
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};

#[tokio::test]
async fn replaced_working_values_survive_workspace_reopen() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    let key = support::qso_key(support::id(10));
    let SaveOutcome::Saved(row) = save::save(
        &store,
        key,
        None,
        support::contact(support::qso(support::id(10))),
    )
    .await?
    else {
        panic!("save")
    };
    let mut qso = support::qso(support::id(10));
    support::edit(&mut qso, |qso| {
        qso.comment = Some("Keep pending values".into());
        Ok(())
    });
    let SaveOutcome::Saved(next) =
        save::save(&store, key, Some(&row), support::contact(qso)).await?
    else {
        panic!("save")
    };
    assert_eq!(save::load(&store, key).await?, Some(next.clone()));
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(save::load(&store, key).await?, Some(next));
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn received_baseline_seals_pending_basis_and_preserves_original_context() -> anyhow::Result<()>
{
    use clacks_client_store::{
        identity::{self, EnrollmentBinding},
        synchronization,
    };
    use clacks_sync::{
        acceptance::{AcceptanceDecision, AcceptanceInput, plan_acceptance},
        account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
        client::{
            LocalCounter, OriginalBasis, PendingRecord, Preparation, preparation::prepare_change,
        },
        membership::{ClientMembership, MembershipSnapshot},
        record::{Observation, RecordState},
        transfer::{RecordRequirement, WindowMode},
    };
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    identity::bind(
        &store,
        None,
        EnrollmentBinding {
            account: support::id(1),
            actor: support::id(9),
            server: "https://example.invalid".into(),
            credential_reference: "protected-reference".into(),
        },
    )
    .await?;
    let (account, baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let mut membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![ClientMembership {
            actor: support::id(9),
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    let key = support::qso_key(support::id(10));
    let absent = Observation::Absent {
        key,
        checkpoint: InstalledCheckpoint {
            account: account.account,
            baseline: baseline.id,
            coverage: account.installed.clone(),
        },
    };
    let bases = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &bases,
    };
    let client = clacks_sync::client::ClientRecordFields {
        key,
        latest: absent.clone(),
        retained: vec![],
        pending: Some(PendingRecord {
            key,
            original: OriginalBasis::NeverCreated {
                key,
                checkpoint: Some(InstalledCheckpoint {
                    account: account.account,
                    baseline: baseline.id,
                    coverage: account.installed.clone(),
                }),
            },
            working: support::contact(support::qso(support::id(10))),
            prepared: None,
            witness: None,
        }),
    }
    .try_into()
    .expect("coherent client fixture");
    let Preparation::Ready { change, .. } = prepare_change(
        &client,
        support::id(9),
        &LocalCounter {
            actor: support::id(9),
            high_water: 0,
        },
        &[],
        &evidence,
    )?
    else {
        panic!("source preparation")
    };
    let AcceptanceDecision::Commit(commit) = plan_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &membership,
            current: &absent,
            references: &[],
        },
        &evidence,
    )?
    else {
        panic!("accepted")
    };
    let source = InstalledCheckpoint {
        account: account.account,
        baseline: baseline.id,
        coverage: commit.next_account.installed.clone(),
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![RecordRequirement {
                key,
                stamp: commit.accepted.stamp.clone(),
                deleted: false,
            }],
            boundaries: vec![],
            baselines: bases.to_vec(),
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };
    let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming
        .page(&store, std::slice::from_ref(&commit.accepted))
        .await?;
    incoming.finish(&store).await?;
    let row = save::load(&store, key).await?.unwrap();
    let mut qso = support::qso(support::id(10));
    support::edit(&mut qso, |qso| {
        qso.comment = Some("Unpublished tail".into());
        Ok(())
    });
    save::save(&store, key, Some(&row), support::contact(qso)).await?;
    membership.clients.first_mut().unwrap().acknowledged = Some(source.clone());
    let retired =
        clacks_sync::membership::revoke(support::id(9), &commit.next_account, &membership)?;
    let received_baseline = clacks_sync::account::Baseline {
        id: support::id(4),
        account: source.account,
        coverage: source.coverage.clone(),
        lineage: baseline.lineage.clone(),
        membership_generation: retired.next_account.membership_generation,
    };
    receive_interpretation(
        &store,
        &source,
        std::slice::from_ref(&received_baseline),
        &received_baseline.lineage,
    )
    .await?;
    let sealed = save::load(&store, key).await?.unwrap();
    let witness = sealed.pending().as_ref().unwrap().witness.as_ref().unwrap();
    assert_eq!(witness.observed(), &commit.accepted.stamp);
    assert_eq!(witness.original(), &sealed.pending().unwrap().original);
    let mut projected = commit.accepted.clone();
    projected.stamp = clacks_sync::record::VersionStamp::new(
        projected.stamp.accepted_at(),
        received_baseline.id,
        projected.stamp.vector().clone(),
    )?;

    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    // Reconcile using the restored sealed basis, rather than in-memory setup.
    synchronization::receive(&store, &projected).await?;
    let retained = save::load(&store, key).await?.unwrap();
    assert!(matches!(
        retained.working_state(),
        Some(RecordState::Live(_))
    ));
    assert_eq!(
        retained.pending().as_ref().unwrap().witness,
        Some(witness.clone())
    );
    // Incompatible meaning for the same baseline cannot replace interpretation or receipt.
    let mut invalid = received_baseline;
    invalid.membership_generation += 1;
    assert!(
        receive_interpretation(&store, &source, &[invalid.clone()], &invalid.lineage)
            .await
            .is_err()
    );
    assert_eq!(save::load(&store, key).await?, Some(retained.clone()));
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(save::load(&store, key).await?, Some(retained));
    store.close().await?;
    Ok(())
}

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

#[tokio::test]
async fn insufficient_installed_coverage_keeps_the_original_without_fabricating_a_witness()
-> anyhow::Result<()> {
    use clacks_client_store::synchronization;
    use clacks_sync::{
        account::{AccountMetadata, InstalledCheckpoint},
        transfer::WindowMode,
    };
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    let (account, baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let checkpoint = InstalledCheckpoint {
        account: account.account,
        baseline: baseline.id,
        coverage: account.installed.clone(),
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: checkpoint.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            boundaries: vec![],
            baselines: vec![baseline.clone()],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };
    let incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming.finish(&store).await?;
    let accepted = support::accepted(1, support::contact(support::qso(support::id(10))));
    synchronization::receive(&store, &accepted).await?;
    let row = save::load(&store, accepted.key()).await?.unwrap();
    let mut qso = support::qso(support::id(10));
    support::edit(&mut qso, |qso| {
        qso.comment = Some("Pending beyond installed receipt".into());
        Ok(())
    });
    let SaveOutcome::Saved(pending) =
        save::save(&store, accepted.key(), Some(&row), support::contact(qso)).await?
    else {
        panic!("saved")
    };
    let received_baseline = clacks_sync::account::Baseline {
        id: support::id(4),
        ..baseline.clone()
    };
    receive_interpretation(&store, &checkpoint, &[received_baseline], &baseline.lineage).await?;
    assert_eq!(save::load(&store, accepted.key()).await?, Some(pending));
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn absence_witness_survives_a_new_baseline_and_workspace_reopen() -> anyhow::Result<()> {
    use clacks_sync::{
        account::{AccountMetadata, InstalledCheckpoint},
        client::OriginalBasis,
        record::{AcceptedRecord, VersionStamp},
        transfer::{RecordRequirement, WindowMode},
    };
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    let (account, old_baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let old_checkpoint = InstalledCheckpoint {
        account: account.account,
        baseline: old_baseline.id,
        coverage: account.installed,
    };
    let initial = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: old_checkpoint.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            boundaries: vec![],
            baselines: vec![old_baseline.clone()],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };
    let incoming = crate::incoming::Incoming::begin(&store, &initial).await?;
    incoming.finish(&store).await?;

    let key = support::qso_key(support::id(10));
    let SaveOutcome::Saved(local) = save::save(
        &store,
        key,
        None,
        support::contact(support::qso(support::id(10))),
    )
    .await?
    else {
        panic!("local save")
    };
    assert!(matches!(&local.pending().unwrap().original,
        OriginalBasis::NeverCreated { checkpoint: Some(checkpoint), .. } if checkpoint.baseline == old_baseline.id));
    let mut next_baseline = old_baseline.clone();
    next_baseline.id = support::id(4);
    let mut remote = support::qso(support::id(10));
    support::edit(&mut remote, |fields| {
        fields.comment = Some("Independent remote creation".into());
        Ok(())
    });
    let remote = support::accepted(1, support::contact(remote));
    let accepted = AcceptedRecord::new(
        key,
        remote.state().clone(),
        VersionStamp::new(
            remote.stamp.accepted_at(),
            next_baseline.id,
            remote.stamp.vector().clone(),
        )?,
    )?;
    let source = InstalledCheckpoint {
        account: account.account,
        baseline: next_baseline.id,
        coverage: clacks_sync::lineage::Coverage::from(accepted.stamp.accepted_at()),
    };
    let manifest = crate::incoming::Source {
        start: clacks_sync::transfer::discovery::DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
        page: clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![RecordRequirement {
                key,
                stamp: accepted.stamp.clone(),
                deleted: false,
            }],
            boundaries: vec![],
            baselines: vec![old_baseline.clone(), next_baseline],
            units: vec![],
            accepted: vec![],
            support: vec![],
        },
    };
    let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
    incoming.page(&store, &[accepted]).await?;
    incoming.finish(&store).await?;
    receive_interpretation(&store, &source, &[], &old_baseline.lineage).await?;
    store.close().await?;

    let store = ClientStore::open(&file, account.account, StoreSettings::default()).await?;
    let restored = save::load(&store, key)
        .await?
        .expect("pending QSO retained");
    let pending = restored
        .pending()
        .expect("independent creations require review");
    let witness = pending
        .witness
        .as_ref()
        .expect("original comparison evidence sealed");
    assert!(
        matches!(witness.original(), OriginalBasis::NeverCreated { checkpoint: Some(checkpoint), .. }
        if checkpoint == &old_checkpoint)
    );
    assert!(
        witness
            .baselines()
            .iter()
            .any(|baseline| baseline.id == old_baseline.id)
    );
    assert_eq!(restored.working_state(), local.working_state());
    store.close().await?;
    Ok(())
}

/// Supply received interpretation through an ordinary incremental page.
async fn receive_interpretation(
    store: &ClientStore,
    checkpoint: &clacks_sync::account::InstalledCheckpoint,
    baselines: &[clacks_sync::account::Baseline],
    lineage: &clacks_sync::lineage::Lineage,
) -> anyhow::Result<()> {
    let mut receipt = clacks_client_store::transfer::begin(
        store,
        &clacks_sync::transfer::discovery::DiscoveryStart {
            source: checkpoint.clone(),
            mode: clacks_sync::transfer::WindowMode::Incremental {
                after: checkpoint.coverage.clone(),
            },
        },
    )
    .await?;
    clacks_client_store::transfer::install_page(
        store,
        &mut receipt,
        &clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            accepted: vec![],
            support: vec![],
            baselines: baselines.to_vec(),
            boundaries: lineage.boundaries().to_vec(),
            units: vec![],
        },
    )
    .await
}
