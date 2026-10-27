use crate::support;
use clacks_client_store::{
    retention,
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};

#[tokio::test]
async fn collection_reclaims_replaced_values_without_losing_pending_work() -> anyhow::Result<()> {
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
    qso.comment = Some("Keep pending values".into());
    let SaveOutcome::Saved(next) =
        save::save(&store, key, Some(&row), support::contact(qso)).await?
    else {
        panic!("save")
    };
    assert!(retention::collect_unreferenced(&store).await? > 0);
    assert_eq!(save::load(&store, key).await?, Some(next.clone()));
    assert_eq!(retention::collect_unreferenced(&store).await?, 0);
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(save::load(&store, key).await?, Some(next));
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn received_compaction_seals_pending_basis_and_preserves_original_context()
-> anyhow::Result<()> {
    use clacks_client_store::{
        identity::{self, EnrollmentBinding},
        synchronization, transfer,
    };
    use clacks_sync::{
        acceptance::{AcceptanceDecision, AcceptanceInput, plan_acceptance},
        account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
        client::{
            ClientRecord, LocalCounter, OriginalBasis, PendingRecord, Preparation, prepare_change,
        },
        compaction::{CompactionInput, plan_baseline},
        membership::{ClientMembership, MembershipSnapshot},
        record::{Observation, RecordState},
        transfer::{RecordRequirement, WindowManifest, WindowMode},
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
    let client = ClientRecord {
        key,
        latest: absent.clone(),
        retained: vec![],
        pending: Some(PendingRecord {
            key,
            original: OriginalBasis::NeverCreated {
                key,
                checkpoint: InstalledCheckpoint {
                    account: account.account,
                    baseline: baseline.id,
                    coverage: account.installed.clone(),
                },
            },
            working: support::contact(support::qso(support::id(10))),
            prepared: None,
            witness: None,
        }),
    };
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
    let manifest = WindowManifest::new(
        account.account,
        source.clone(),
        WindowMode::Full,
        vec![RecordRequirement {
            key,
            stamp: commit.accepted.stamp.clone(),
            deleted: false,
        }],
        vec![],
        bases.to_vec(),
        vec![],
    )?;
    transfer::remember(&store, &manifest).await?;
    transfer::install(
        &store,
        &source,
        &WindowMode::Full,
        std::slice::from_ref(&commit.accepted),
    )
    .await?;
    transfer::finish(&store, &source, &WindowMode::Full).await?;
    let row = save::load(&store, key).await?.unwrap();
    let mut qso = support::qso(support::id(10));
    qso.comment = Some("Unpublished tail".into());
    save::save(&store, key, Some(&row), support::contact(qso)).await?;
    membership.clients.first_mut().unwrap().acknowledged = Some(source.clone());
    let retired =
        clacks_sync::membership::revoke(support::id(9), &commit.next_account, &membership)?;
    let baseline_plan = plan_baseline(
        support::id(4),
        &CompactionInput {
            account: &retired.next_account,
            membership: &retired.next,
            records: std::slice::from_ref(&commit.accepted),
            surviving: &[],
        },
        &evidence,
    )?;
    assert!(baseline_plan.removable_clients.contains(&support::id(9)));
    retention::install_interpretation(
        &store,
        &source,
        std::slice::from_ref(&baseline_plan.baseline),
        &baseline_plan.baseline.lineage,
    )
    .await?;
    let sealed = save::load(&store, key).await?.unwrap();
    let witness = sealed.pending.as_ref().unwrap().witness.as_ref().unwrap();
    assert_eq!(witness.observed, commit.accepted.stamp);
    assert_eq!(witness.original, sealed.pending.as_ref().unwrap().original);
    let projected = baseline_plan.projected.first().unwrap();
    synchronization::receive(&store, projected).await?;
    retention::collect_unreferenced(&store).await?;
    let retained = save::load(&store, key).await?.unwrap();
    assert!(matches!(
        retained.working_state(),
        Some(RecordState::Live(_))
    ));
    assert_eq!(
        retained.pending.as_ref().unwrap().witness,
        Some(witness.clone())
    );
    // Incompatible meaning for the same baseline cannot replace interpretation or receipt.
    let mut invalid = baseline_plan.baseline;
    invalid.membership_generation += 1;
    assert!(
        retention::install_interpretation(&store, &source, &[invalid.clone()], &invalid.lineage)
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
    use clacks_client_store::{read, transfer};
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
        transfer::{RecordRequirement, WindowManifest, WindowMode},
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
        coverage: Coverage::from(accepted.stamp.accepted_at),
    };
    let requirement = RecordRequirement {
        key: accepted.key,
        stamp: accepted.stamp.clone(),
        deleted: false,
    };
    let manifest = WindowManifest::new(
        restored.account,
        completed.clone(),
        WindowMode::Full,
        vec![requirement.clone()],
        vec![],
        vec![baseline.clone()],
        vec![],
    )?;
    transfer::remember(&store, &manifest).await?;
    transfer::install(
        &store,
        &completed,
        &WindowMode::Full,
        std::slice::from_ref(&accepted),
    )
    .await?;
    transfer::finish(&store, &completed, &WindowMode::Full).await?;
    let row = save::load(&store, accepted.key).await?.unwrap();
    let mut qso = support::qso(support::id(10));
    qso.comment = Some("Keep local changes across server restoration".into());
    save::save(&store, accepted.key, Some(&row), support::contact(qso)).await?;
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
    retention::install_interpretation(&store, &completed, &[], &ancestry).await?;
    let retained = save::load(&store, accepted.key).await?.unwrap();
    assert_eq!(retained.latest, Observation::Accepted(accepted.clone()));
    let evidence = EvidenceView {
        lineage: &ancestry,
        baselines: &bases,
    };
    let absence = Observation::Absent {
        key: accepted.key,
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
    let recovered = WindowManifest::new(
        restored.account,
        source.clone(),
        WindowMode::Full,
        vec![requirement],
        vec![fork.boundary],
        bases.to_vec(),
        vec![],
    )?;
    transfer::remember(&store, &recovered).await?;
    transfer::install(&store, &source, &WindowMode::Full, &[envelope]).await?;
    let receipt = transfer::finish(&store, &source, &WindowMode::Full).await?;
    assert!(receipt.coverage.covers(accepted.stamp.accepted_at));
    retention::collect_unreferenced(&store).await?;
    assert_eq!(
        save::load(&store, accepted.key)
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
        snapshot.record(accepted.key).await?.unwrap().latest,
        Observation::Accepted(accepted)
    );
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn insufficient_installed_coverage_keeps_the_original_without_fabricating_a_witness()
-> anyhow::Result<()> {
    use clacks_client_store::{synchronization, transfer};
    use clacks_sync::{
        account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
        compaction::{CompactionInput, plan_baseline},
        membership::MembershipSnapshot,
        transfer::{WindowManifest, WindowMode},
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
    let manifest = WindowManifest::new(
        account.account,
        checkpoint.clone(),
        WindowMode::Full,
        vec![],
        vec![],
        vec![baseline.clone()],
        vec![],
    )?;
    transfer::remember(&store, &manifest).await?;
    transfer::finish(&store, &checkpoint, &WindowMode::Full).await?;
    let accepted = support::accepted(1, support::contact(support::qso(support::id(10))));
    synchronization::receive(&store, &accepted).await?;
    let row = save::load(&store, accepted.key).await?.unwrap();
    let mut qso = support::qso(support::id(10));
    qso.comment = Some("Pending beyond installed receipt".into());
    let SaveOutcome::Saved(pending) =
        save::save(&store, accepted.key, Some(&row), support::contact(qso)).await?
    else {
        panic!("saved")
    };
    let bases = [baseline.clone()];
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let compacted = plan_baseline(
        support::id(4),
        &CompactionInput {
            account: &account,
            membership: &membership,
            records: &[],
            surviving: &[],
        },
        &EvidenceView {
            lineage: &baseline.lineage,
            baselines: &bases,
        },
    )?;
    retention::install_interpretation(
        &store,
        &checkpoint,
        &[compacted.baseline],
        &baseline.lineage,
    )
    .await?;
    retention::collect_unreferenced(&store).await?;
    assert_eq!(save::load(&store, accepted.key).await?, Some(pending));
    store.close().await?;
    Ok(())
}
