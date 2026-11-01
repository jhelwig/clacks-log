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
    let (first, concurrent) = tokio::join!(
        crate::store::retention::collect(&native.store, key.account),
        crate::store::retention::collect(&native.store, key.account),
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
    crate::store::retention::collect(&native.store, key.account).await?;
    let mut read = native.store.pool.acquire().await?;
    assert_eq!(
        account::load(&mut read, key.account).await?,
        installed,
        "idle collection must not create a revision feedback loop"
    );
    let captured = snapshot.snapshot().into_vec();
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
