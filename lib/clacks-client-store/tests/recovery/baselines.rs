use crate::support;
use clacks_client_store::{
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};

use super::receive_interpretation;

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
    let synchronization::PrepareOutcome::Core(prepared) =
        synchronization::prepare(&store, key).await?
    else {
        panic!("enrolled preparation");
    };
    assert!(matches!(*prepared, Preparation::Ready { .. }));

    let frozen = save::load(&store, key).await?;

    clacks_client_store::retention::reclaim_completed_cycle(&store).await?;
    assert_eq!(
        save::load(&store, key).await?,
        frozen,
        "the original, witness and frozen proposal all remain rooted"
    );

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
    let mut invalid = received_baseline.clone();
    invalid.membership_generation += 1;
    assert!(
        receive_interpretation(&store, &source, &[invalid.clone()], &invalid.lineage)
            .await
            .is_err()
    );
    assert_eq!(save::load(&store, key).await?, Some(retained.clone()));

    store.close().await?;

    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(save::load(&store, key).await?, Some(retained.clone()));

    let previous = retained.pending().unwrap().prepared.as_ref().unwrap();
    let mut server = retired.next_account.clone();
    server.baseline = received_baseline.id;
    let interpretations = [baseline.clone(), received_baseline];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &interpretations,
    };
    let clacks_sync::record::Observation::Accepted(shared) = retained.latest() else {
        panic!("received accepted basis");
    };

    let AcceptanceDecision::Commit(metadata_update) =
        clacks_sync::acceptance::plan_server_acceptance(
            &clacks_sync::acceptance::ServerChange {
                key,
                state: shared.state().clone(),
                expected: clacks_sync::client::VersionCondition::from_observation(
                    retained.latest(),
                )?,
                dependencies: vec![],
            },
            &AcceptanceInput {
                account: &server,
                membership: &retired.next,
                current: retained.latest(),
                references: &[],
                destination_registry: None,
                bindings: &[],
            },
            &evidence,
        )?
    else {
        panic!("server metadata update");
    };

    synchronization::receive(&store, &metadata_update.accepted).await?;

    clacks_client_store::retention::reclaim_completed_cycle(&store).await?;

    store.close().await?;

    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    let synchronization::PrepareOutcome::Core(next) = synchronization::prepare(&store, key).await?
    else {
        panic!("enrolled preparation");
    };

    let Preparation::Ready { change: next, .. } = *next else {
        panic!("settled proposal must reserve a fresh counter");
    };

    let actor = clacks_sync::clock::ClockActor::Client(support::id(9));
    assert!(next.vector().component(actor) > previous.vector().component(actor));
    assert_eq!(next.state(), previous.state());

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

    let retained_before_cleanup = save::load(&store, key).await?;

    let mut review_snapshot = clacks_client_store::read::snapshot(&store).await?;
    let review_before_cleanup = review_snapshot.review_record(key).await?;
    assert!(review_before_cleanup.as_ref().unwrap().review.is_some());
    drop(review_snapshot);

    let checkpoint_before_cleanup = clacks_client_store::synchronization::state(&store)
        .await?
        .checkpoint;

    clacks_client_store::retention::reclaim_completed_cycle(&store).await?;
    assert_eq!(save::load(&store, key).await?, retained_before_cleanup);

    let mut review_snapshot = clacks_client_store::read::snapshot(&store).await?;
    assert_eq!(
        review_snapshot.review_record(key).await?,
        review_before_cleanup
    );
    drop(review_snapshot);
    assert_eq!(
        clacks_client_store::synchronization::state(&store)
            .await?
            .checkpoint,
        checkpoint_before_cleanup
    );

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
