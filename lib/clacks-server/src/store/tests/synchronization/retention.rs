//! Retention obligations across real account storage and discovery.

use super::super::support::synchronization::{NativeAccount, qso};
use crate::store::{discovery::Source, metadata::account, synchronization};
use clacks_model::ids::QsoId;
use clacks_sync::{
    acceptance::AcceptanceDecision,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};

#[tokio::test]
async fn retention_journal_pages_shared_installation_revision() -> anyhow::Result<()> {
    let native = NativeAccount::create().await?;
    let mut connection = native.store.pool.acquire().await?;
    let before = account::load(&mut connection, native.first.account).await?;
    drop(connection);
    let checkpoint = synchronization::checkpoint(&before);
    let mut keys = Vec::new();
    for _ in 0..3 {
        let contact = qso(QsoId::new(), Default::default());
        let key = RecordKey {
            account: native.first.account,
            id: RecordId::Qso(contact.id()),
        };
        let change = native
            .prepare(
                &native.first,
                key,
                RecordState::Live(RecordValue::Qso(Box::new(contact))),
            )
            .await?;
        assert!(matches!(
            synchronization::accept_pending(&native.store, &native.first, &change).await?,
            AcceptanceDecision::Commit(_)
        ));
        keys.push(key);
    }

    // A baseline installation can journal several existing heads at one revision.
    // Author that valid stored input using the accepted records' real stamps.
    let fixtures = sqlx::PgPool::connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?).await?;
    sqlx::query_file!(
        "tests/sql/shared_installation_revision.sql",
        native.first.account.to_string(),
        i64::try_from(before.head.revision)?
    )
    .execute(&fixtures)
    .await?;
    fixtures.close().await;
    let mut source = Source::start(&native.store, &native.first, Some(&checkpoint)).await?;
    assert!(source.end().is_err());
    let mut received = Vec::new();
    while let Some(page) = source.page(&native.store, &native.first, 1).await? {
        received.extend(page.requirements.into_iter().map(|item| item.key));
    }
    received.sort();
    keys.sort();
    assert_eq!(
        received, keys,
        "one installation must not lose entries at a page boundary"
    );
    assert_eq!(source.end()?.pages, 3);
    let mut overtaken = Source::start(&native.store, &native.first, Some(&checkpoint)).await?;
    crate::store::retention::collect(
        &native.store,
        native.first.account,
        1,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await?;
    assert!(
        overtaken
            .page(&native.store, &native.first, 1)
            .await
            .is_err(),
        "removing part of one revision invalidates its entire incremental interval"
    );
    assert!(matches!(
        Source::start(&native.store, &native.first, Some(&checkpoint))
            .await?
            .start
            .mode,
        clacks_sync::transfer::WindowMode::Full
    ));
    Ok(())
}

#[tokio::test]
async fn retention_installation_preserves_versions_and_is_idle_stable() -> anyhow::Result<()> {
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshot = recorder.snapshotter();
    let _metrics = metrics::set_default_local_recorder(&recorder);
    let native = NativeAccount::create().await?;
    let contact = qso(QsoId::new(), Default::default());
    let key = RecordKey {
        account: native.first.account,
        id: RecordId::Qso(contact.id()),
    };
    let change = native
        .prepare(
            &native.first,
            key,
            RecordState::Live(RecordValue::Qso(Box::new(contact))),
        )
        .await?;
    let AcceptanceDecision::Commit(original) =
        synchronization::accept_pending(&native.store, &native.first, &change).await?
    else {
        anyhow::bail!("QSO not accepted");
    };
    crate::enrollment::revoke_in_store(&native.store.pool, &native.principal, native.first.actor)
        .await?;
    let mut read = native.store.pool.acquire().await?;
    let before = account::load(&mut read, key.account).await?;
    drop(read);
    synchronization::acknowledge(
        &native.store,
        &native.second,
        &synchronization::checkpoint(&before),
    )
    .await?;
    let stop = tokio_util::sync::CancellationToken::new();
    let (first, concurrent) = tokio::join!(
        crate::store::retention::collect(&native.store, key.account, 10_000, &stop),
        crate::store::retention::collect(&native.store, key.account, 10_000, &stop),
    );
    first?;
    concurrent?;
    let mut read = native.store.pool.acquire().await?;
    let installed = account::load(&mut read, key.account).await?;
    let evidence = account::interpretation(&mut read, key.account).await?;
    let clacks_sync::record::Observation::Accepted(projected) =
        synchronization::context::observation(&mut read, &installed, key).await?
    else {
        anyhow::bail!("current QSO disappeared");
    };
    assert_eq!(original.accepted.state(), projected.state());
    assert_eq!(
        original.accepted.stamp.accepted_at(),
        projected.stamp.accepted_at()
    );
    assert_eq!(
        clacks_sync::compare::compare_records(&original.accepted, &projected, &evidence.view())?,
        clacks_sync::compare::RecordRelation::SameVersion
    );
    assert_eq!(installed.head.revision, before.head.revision + 1);
    assert_ne!(installed.baseline, before.baseline);
    drop(read);
    let returned = synchronization::receipt::receive(
        &native.store,
        &native.second,
        &clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            accepted: vec![original.accepted.clone()],
            support: vec![],
            baselines: evidence.baselines,
            boundaries: vec![],
            units: vec![],
        },
    )
    .await?;
    assert!(returned.reviews.is_empty());
    crate::store::retention::collect(
        &native.store,
        key.account,
        10_000,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await?;
    let mut read = native.store.pool.acquire().await?;
    assert_eq!(
        account::load(&mut read, key.account).await?,
        installed,
        "idle collection must not create a revision feedback loop"
    );
    let captured = snapshot.snapshot().into_vec();
    for (name, expected) in [
        ("clacks_sync_journal_prune_duration_seconds", "unchanged"),
        ("clacks_sync_metadata_reclaim_duration_seconds", "reclaimed"),
        ("clacks_sync_metadata_reclaim_duration_seconds", "unchanged"),
    ] {
        assert!(
            captured
                .iter()
                .any(|(key, _, _, _)| key.key().name() == name
                    && key
                        .key()
                        .labels()
                        .any(|label| label.key() == "outcome" && label.value() == expected)),
            "missing {name} outcome {expected}"
        );
    }
    for (key, _, _, value) in &captured {
        if key.key().name().starts_with("clacks_sync_baseline_") {
            eprintln!(
                "{} {:?}: {value:?}",
                key.key().name(),
                key.key().labels().collect::<Vec<_>>()
            );
        }
    }
    let (key, _, _, count) = captured
        .iter()
        .find(|(key, _, _, _)| key.key().name() == "clacks_sync_baseline_contexts_converted_total")
        .unwrap();
    assert_eq!(key.key().labels().count(), 0);
    assert!(matches!(
        count,
        metrics_util::debugging::DebugValue::Counter(1)
    ));
    Ok(())
}

#[tokio::test]
async fn retention_prunes_installations_and_returns_below_floor() -> anyhow::Result<()> {
    use clacks_sync::transfer::WindowMode;
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshot = recorder.snapshotter();
    let _metrics = metrics::set_default_local_recorder(&recorder);
    let native = NativeAccount::create().await?;
    let mut read = native.store.pool.acquire().await?;
    let initial = account::load(&mut read, native.first.account).await?;
    drop(read);
    let checkpoint = synchronization::checkpoint(&initial);
    let mut checkpoints = Vec::new();
    let mut oldest = None;
    for _ in 0..5 {
        let contact = qso(QsoId::new(), Default::default());
        let key = RecordKey {
            account: native.first.account,
            id: RecordId::Qso(contact.id()),
        };
        let change = native
            .prepare(
                &native.first,
                key,
                RecordState::Live(RecordValue::Qso(Box::new(contact))),
            )
            .await?;
        let AcceptanceDecision::Commit(commit) =
            synchronization::accept_pending(&native.store, &native.first, &change).await?
        else {
            anyhow::bail!("new QSO was not accepted");
        };
        checkpoints.push(synchronization::checkpoint(&commit.next_account));
        oldest.get_or_insert(commit.accepted);
    }
    let oldest = oldest.unwrap();
    let RecordState::Live(RecordValue::Qso(contact)) = oldest.state() else {
        panic!("live QSO");
    };
    let mut fields = contact.as_ref().clone().into_fields();
    fields.comment = Some("a later correction".into());
    accept(
        &native,
        oldest.key(),
        RecordState::Live(RecordValue::Qso(Box::new(fields.try_into()?))),
    )
    .await?;
    let mut in_flight = Source::start(&native.store, &native.second, Some(&checkpoint)).await?;
    assert!(matches!(
        in_flight.start.mode,
        WindowMode::Incremental { .. }
    ));
    crate::store::retention::collect(
        &native.store,
        native.first.account,
        3,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await?;
    let mut boundaries = checkpoints.iter().skip(1);
    let before_cutoff = boundaries.next().unwrap();
    let cutoff = boundaries.next().unwrap();
    assert!(matches!(
        Source::start(&native.store, &native.second, Some(before_cutoff))
            .await?
            .start
            .mode,
        WindowMode::Full
    ));
    let mut retained = Source::start(&native.store, &native.second, Some(cutoff)).await?;
    assert!(matches!(
        retained.start.mode,
        WindowMode::Incremental { .. }
    ));
    let mut retained_entries = 0;
    while let Some(page) = retained.page(&native.store, &native.second, 1).await? {
        retained_entries += page.requirements.len();
    }
    assert_eq!(retained_entries, 3);
    assert!(snapshot.snapshot().into_vec().iter().any(|(key, _, _, _)| {
        key.key().name() == "clacks_sync_journal_prune_duration_seconds"
            && key
                .key()
                .labels()
                .any(|label| label.key() == "outcome" && label.value() == "pruned")
    }));
    let mut read = native.store.pool.acquire().await?;
    assert!(
        synchronization::context::exact(
            &mut read,
            &clacks_sync::service::VersionReference::from(&oldest)
        )
        .await?
        .is_none(),
        "preserving ordering does not pin the obsolete QSO body"
    );
    drop(read);
    assert!(
        in_flight
            .page(&native.store, &native.second, 1)
            .await
            .is_err()
    );
    assert!(in_flight.end().is_err());
    let mut retry = Source::start(&native.store, &native.second, Some(&checkpoint)).await?;
    assert!(matches!(retry.start.mode, WindowMode::Full));
    let mut qsos = 0;
    while let Some(page) = retry.page(&native.store, &native.second, 2).await? {
        qsos += page
            .accepted
            .iter()
            .filter(|value| matches!(value.key().id, RecordId::Qso(_)))
            .count();
    }
    assert_eq!(qsos, 5);
    Ok(())
}

async fn accept(
    native: &NativeAccount,
    key: RecordKey,
    state: RecordState,
) -> anyhow::Result<clacks_sync::record::AcceptedRecord> {
    let proposal = native.prepare(&native.first, key, state).await?;
    let AcceptanceDecision::Commit(commit) =
        synchronization::accept_pending(&native.store, &native.first, &proposal).await?
    else {
        anyhow::bail!("retention fixture change was not accepted");
    };
    Ok(commit.accepted)
}

#[tokio::test]
async fn retention_reclaims_bodies_without_losing_obligations() -> anyhow::Result<()> {
    use clacks_sync::{record::Observation, service::VersionReference};
    let native = NativeAccount::create().await?;
    let mut read = native.store.pool.acquire().await?;
    let before = account::load(&mut read, native.first.account).await?;
    drop(read);
    let initial_checkpoint = synchronization::checkpoint(&before);
    let mut old = Vec::new();
    let mut deleted = None;
    for remove in [false, true] {
        let contact = qso(QsoId::new(), Default::default());
        let key = RecordKey {
            account: native.first.account,
            id: RecordId::Qso(contact.id()),
        };
        old.push(
            accept(
                &native,
                key,
                RecordState::Live(RecordValue::Qso(Box::new(contact.clone()))),
            )
            .await?,
        );
        let next = if remove {
            RecordState::Deleted
        } else {
            let mut fields = contact.into_fields();
            fields.comment = Some("corrected on-air information".into());
            RecordState::Live(RecordValue::Qso(Box::new(fields.try_into()?)))
        };
        let accepted = accept(&native, key, next).await?;
        if remove {
            deleted = Some(accepted);
        }
    }
    let deleted = deleted.unwrap();
    crate::enrollment::revoke_in_store(&native.store.pool, &native.principal, native.first.actor)
        .await?;
    let mut read = native.store.pool.acquire().await?;
    let state = account::load(&mut read, native.first.account).await?;
    drop(read);
    synchronization::acknowledge(
        &native.store,
        &native.second,
        &synchronization::checkpoint(&state),
    )
    .await?;
    crate::store::retention::collect(
        &native.store,
        native.first.account,
        10_000,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await?;
    let mut read = native.store.pool.acquire().await?;
    for obsolete in &old {
        assert!(
            synchronization::context::exact(&mut read, &VersionReference::from(obsolete))
                .await?
                .is_none(),
            "a journal descriptor must not root an otherwise unused QSO body"
        );
    }
    assert!(
        synchronization::context::exact(&mut read, &VersionReference::from(&deleted))
            .await?
            .is_none(),
        "an equivalent current tombstone replaces the old envelope representation"
    );
    let current = account::load(&mut read, native.first.account).await?;
    assert!(
        matches!(synchronization::context::observation(&mut read, &current, deleted.key()).await?, Observation::Accepted(value) if matches!(value.state(), RecordState::Deleted))
    );
    drop(read);
    let mut source =
        Source::start(&native.store, &native.second, Some(&initial_checkpoint)).await?;
    let mut requirements = Vec::new();
    while let Some(page) = source.page(&native.store, &native.second, 1).await? {
        requirements.extend(page.requirements);
    }
    for original in old {
        assert!(
            requirements
                .iter()
                .any(|required| required.key == original.key()
                    && required.stamp.accepted_at() == original.stamp.accepted_at()
                    && !required.deleted)
        );
    }
    assert!(
        requirements
            .iter()
            .any(|required| required.key == deleted.key()
                && required.stamp.accepted_at() == deleted.stamp.accepted_at()
                && required.deleted)
    );
    Ok(())
}
