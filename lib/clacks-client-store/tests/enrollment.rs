//! Enrollment replacement preserves local work until an explicit admission review.

use crate::support;
use clacks_client_store::{
    enrollment::{self, RecoveryDisposition},
    identity::{self, EnrollmentBinding},
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
    synchronization::{self, PrepareOutcome},
    transfer,
};
use clacks_sync::{
    account::{AccountMetadata, InstalledCheckpoint},
    client::{Preparation, ReviewReason},
    lineage::Coverage,
    membership::{ClientMembership, MembershipSnapshot},
    transfer::{RecordRequirement, WindowManifest, WindowMode},
};

#[tokio::test]
async fn fresh_actor_holds_old_work_until_explicit_review() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("account.sqlite");
    let store = ClientStore::create(&path, support::id(1), StoreSettings::default()).await?;
    let old = EnrollmentBinding {
        account: support::id(1),
        actor: support::id(9),
        server: "https://clacks.example".into(),
        credential_reference: "old-native-entry".into(),
    };
    identity::bind(&store, None, old.clone()).await?;
    let unsubmitted = support::qso_key(support::id(30));
    save::save(
        &store,
        unsubmitted,
        None,
        support::contact(support::qso(support::id(30))),
    )
    .await?;

    let (mut account, baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let checkpoint = InstalledCheckpoint {
        account: account.account,
        baseline: baseline.id,
        coverage: account.installed.clone(),
    };
    let empty = WindowManifest::new(
        account.account,
        checkpoint.clone(),
        WindowMode::Full,
        vec![],
        vec![],
        vec![baseline.clone()],
        vec![],
    )?;
    transfer::remember(&store, &empty).await?;
    transfer::finish(&store, &checkpoint, &WindowMode::Full).await?;
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![ClientMembership {
            actor: old.actor,
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    synchronization::observe_account(&store, None, &account, &membership).await?;
    let key = support::qso_key(support::id(10));
    save::save(
        &store,
        key,
        None,
        support::contact(support::qso(support::id(10))),
    )
    .await?;
    assert!(
        matches!(synchronization::prepare(&store,key).await?,PrepareOutcome::Core(value) if matches!(*value,Preparation::Ready { .. }))
    );
    let frozen = save::load(&store, key).await?.unwrap();

    let same = EnrollmentBinding {
        credential_reference: "replacement-native-entry".into(),
        ..old.clone()
    };
    enrollment::install(
        &store,
        Some(&old),
        same.clone(),
        RecoveryDisposition::SameActor,
    )
    .await?;
    assert_eq!(save::load(&store, key).await?, Some(frozen.clone()));
    let fresh = EnrollmentBinding {
        actor: support::id(11),
        credential_reference: "fresh-native-entry".into(),
        ..same.clone()
    };
    enrollment::install(
        &store,
        Some(&same),
        fresh.clone(),
        RecoveryDisposition::FreshActor {
            former_actor: old.actor,
        },
    )
    .await?;
    store.close().await?;
    let store = ClientStore::open(&path, support::id(1), StoreSettings::default()).await?;
    assert_eq!(identity::binding(&store).await?, Some(fresh.clone()));
    assert_eq!(save::load(&store, key).await?, Some(frozen.clone()));
    assert!(
        matches!(synchronization::prepare(&store,key).await?,PrepareOutcome::AuthorizationReview { former_actor } if former_actor == old.actor)
    );
    let mut edited = support::qso(support::id(10));
    support::edit(&mut edited, |edited| {
        edited.comment = Some("Keep this correction".into());
        Ok(())
    });
    save::save(&store, key, Some(&frozen), support::contact(edited)).await?;
    let pending = save::load(&store, key).await?.unwrap();
    assert_eq!(
        pending.pending().as_ref().unwrap().prepared,
        frozen.pending().as_ref().unwrap().prepared
    );
    assert!(matches!(
        synchronization::prepare(&store, key).await?,
        PrepareOutcome::AuthorizationReview { .. }
    ));

    let accepted = support::accepted(1, support::contact(support::qso(support::id(10))));
    let source = InstalledCheckpoint {
        coverage: Coverage::from(accepted.stamp.accepted_at()),
        ..checkpoint
    };
    let window = WindowManifest::new(
        account.account,
        source.clone(),
        WindowMode::Full,
        vec![RecordRequirement {
            key,
            stamp: accepted.stamp.clone(),
            deleted: false,
        }],
        vec![],
        vec![baseline],
        vec![],
    )?;
    transfer::remember(&store, &window).await?;
    transfer::install(
        &store,
        &source,
        &WindowMode::Full,
        std::slice::from_ref(&accepted),
    )
    .await?;
    transfer::finish(&store, &source, &WindowMode::Full).await?;
    let mut snapshot = clacks_client_store::read::snapshot(&store).await?;
    let reviews = snapshot.review(&Default::default()).await?;
    let shown = reviews
        .iter()
        .find(|row| row.record.key() == key)
        .and_then(|row| row.review.as_ref())
        .unwrap();
    assert_eq!(shown.reason, ReviewReason::ExcludedActor(old.actor));
    let absent_review = reviews
        .iter()
        .find(|row| row.record.key() == unsubmitted)
        .and_then(|row| row.review.as_ref())
        .cloned()
        .expect("unsubmitted contact has no resolvable authorization review");
    assert_eq!(absent_review.reason, ReviewReason::ExcludedActor(old.actor));
    let unsent = save::load(&store, unsubmitted).await?.unwrap();
    assert!(matches!(
        unsent.latest(),
        clacks_sync::record::Observation::Absent { .. }
    ));

    let current = save::load(&store, key).await?.unwrap();
    assert_eq!(
        current.pending().as_ref().unwrap().prepared,
        frozen.pending().as_ref().unwrap().prepared
    );
    let chosen = current.working_state().unwrap().clone();
    let shown = shown.clone();
    snapshot.close().await?;
    assert!(matches!(
        synchronization::resolve(&store, &current, &shown, chosen).await?,
        SaveOutcome::Saved(_)
    ));

    let chosen_unsent = unsent.working_state().unwrap().clone();
    assert!(matches!(
        synchronization::resolve(&store, &unsent, &absent_review, chosen_unsent).await?,
        SaveOutcome::Saved(_)
    ));

    account.head = accepted.stamp.accepted_at();
    account.installed = source.coverage;
    account.membership_generation = 1;
    let next = MembershipSnapshot {
        account: account.account,
        generation: 1,
        clients: vec![ClientMembership {
            actor: fresh.actor,
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    synchronization::observe_account(
        &store,
        Some(&(
            AccountMetadata::initialize(support::id(1), support::id(2), support::id(3)).0,
            membership,
        )),
        &account,
        &next,
    )
    .await?;
    assert!(
        matches!(synchronization::prepare(&store,key).await?,PrepareOutcome::Core(value) if matches!(*value,Preparation::Ready { .. }))
    );
    assert!(
        matches!(synchronization::prepare(&store, unsubmitted).await?, PrepareOutcome::Core(value) if matches!(*value, Preparation::Ready { .. }))
    );
    let unrelated = support::qso_key(support::id(20));
    save::save(
        &store,
        unrelated,
        None,
        support::contact(support::qso(support::id(20))),
    )
    .await?;
    assert!(
        matches!(synchronization::prepare(&store, unrelated).await?,PrepareOutcome::Core(value) if matches!(*value,Preparation::Ready { .. }))
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn completion_attempt_and_binding_are_guarded_across_restart() -> anyhow::Result<()> {
    use enrollment::EnrollmentAttempt;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("account.sqlite");
    let store = ClientStore::create(&path, support::id(1), StoreSettings::default()).await?;
    let binding = EnrollmentBinding {
        account: support::id(1),
        actor: support::id(9),
        server: "https://clacks.example".into(),
        credential_reference: "saved-native-entry".into(),
    };
    let mut saved = EnrollmentAttempt::new(
        binding.credential_reference.clone(),
        binding.server.clone(),
        Some(directory.path().into()),
        None,
    )?;
    saved.credential_saved();
    enrollment::save_attempt(&store, &saved).await?;
    store.close().await?;
    let store = ClientStore::open(&path, support::id(1), StoreSettings::default()).await?;
    assert_eq!(enrollment::attempt(&store).await?, Some(saved.clone()));
    let mut stale = EnrollmentAttempt::new(
        "superseded-native-entry".into(),
        binding.server.clone(),
        Some(directory.path().into()),
        None,
    )?;
    stale.credential_saved();
    enrollment::clear_attempt(&store, &stale).await?;
    assert_eq!(enrollment::attempt(&store).await?, Some(saved.clone()));
    let wrong = EnrollmentBinding {
        account: support::id(2),
        ..binding.clone()
    };
    assert!(
        enrollment::install(&store, None, wrong, RecoveryDisposition::New)
            .await
            .is_err()
    );
    assert!(identity::binding(&store).await?.is_none());
    enrollment::install(&store, None, binding.clone(), RecoveryDisposition::New).await?;
    let replacement = EnrollmentBinding {
        credential_reference: "replacement-native-entry".into(),
        ..binding.clone()
    };
    assert!(
        enrollment::install(&store, None, replacement.clone(), RecoveryDisposition::New)
            .await
            .is_err()
    );
    assert_eq!(identity::binding(&store).await?, Some(binding.clone()));
    enrollment::install(
        &store,
        Some(&binding),
        replacement.clone(),
        RecoveryDisposition::SameActor,
    )
    .await?;
    assert!(
        enrollment::install(
            &store,
            Some(&binding),
            binding.clone(),
            RecoveryDisposition::SameActor
        )
        .await
        .is_err()
    );
    assert_eq!(identity::binding(&store).await?, Some(replacement));
    enrollment::clear_attempt(&store, &saved).await?;
    assert!(enrollment::attempt(&store).await?.is_none());
    store.close().await?;
    Ok(())
}
