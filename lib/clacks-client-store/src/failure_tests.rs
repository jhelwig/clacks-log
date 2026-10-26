//! Real SQLite failure boundaries exercised through production store workflows.

#[path = "../tests/support/process.rs"]
mod process;
#[path = "../tests/support/mod.rs"]
mod support;
#[path = "../tests/support/telemetry.rs"]
mod telemetry;
use crate::{
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use sqlx::Executor;

#[tokio::test]
async fn busy_and_cancelled_transactions_preserve_previous_values() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings {
            busy_timeout: std::time::Duration::from_millis(30),
            ..Default::default()
        },
    )
    .await?;
    let key = support::qso_key(support::id(10));
    let original = support::contact(support::qso(support::id(10)));
    let held = store.write().await?;
    assert!(
        save::save(&store, key, None, original.clone())
            .await
            .is_err()
    );
    held.rollback().await?;
    assert!(save::load(&store, key).await?.is_none());
    let task_store = store.clone();
    let state = original.clone();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut tx = task_store.write().await.unwrap();
        save::save_in(&mut tx, key, None, state).await.unwrap();
        sender.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    receiver.await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let SaveOutcome::Saved(saved) = save::save(&store, key, None, original).await? else {
        panic!("cancelled transaction must roll back")
    };
    assert_eq!(save::load(&store, key).await?, Some(saved));
    store.close().await?;
    Ok(())
}

async fn full_write_and_recovery() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings {
            max_connections: 1,
            ..Default::default()
        },
    )
    .await?;
    let mut connection = store.pool.acquire().await?;
    connection.execute("PRAGMA max_page_count = 128").await?;
    drop(connection);
    let key = support::qso_key(support::id(10));
    let mut qso = support::qso(support::id(10));
    qso.comment = Some("valid recorded text ".repeat(500_000));
    let error = save::save(&store, key, None, support::contact(qso))
        .await
        .unwrap_err();
    assert!(error.chain().any(|cause| {
        cause
            .downcast_ref::<sqlx::Error>()
            .and_then(sqlx::Error::as_database_error)
            .is_some_and(|cause| cause.code().as_deref() == Some("13"))
    }));
    assert!(save::load(&store, key).await?.is_none());
    // Restore capacity on the same pooled connection and verify that no failed
    // identity/payload fragments prevent a subsequent normal save.
    let mut connection = store.pool.acquire().await?;
    connection
        .execute("PRAGMA max_page_count = 1073741823")
        .await?;
    drop(connection);
    assert!(matches!(
        save::save(
            &store,
            key,
            None,
            support::contact(support::qso(support::id(10)))
        )
        .await?,
        SaveOutcome::Saved(_)
    ));
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn readonly_and_corrupt_files_do_not_report_a_successful_save_or_open() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(
        &file,
        support::id(1),
        StoreSettings {
            max_connections: 1,
            ..Default::default()
        },
    )
    .await?;
    let mut connection = store.pool.acquire().await?;
    connection.execute("PRAGMA query_only = ON").await?;
    drop(connection);
    assert!(
        save::save(
            &store,
            support::qso_key(support::id(10)),
            None,
            support::contact(support::qso(support::id(10)))
        )
        .await
        .is_err()
    );
    let mut connection = store.pool.acquire().await?;
    connection.execute("PRAGMA query_only = OFF").await?;
    drop(connection);
    assert!(
        save::load(&store, support::qso_key(support::id(10)))
            .await?
            .is_none()
    );
    store.close().await?;
    std::fs::write(&file, b"deliberately invalid database header")?;
    let before = std::fs::read(&file)?;
    assert!(
        ClientStore::open(&file, support::id(1), StoreSettings::default())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[test]
#[ignore = "invoked only by the parent interruption test"]
fn transaction_child() -> anyhow::Result<()> {
    let file = std::path::PathBuf::from(
        std::env::var_os("CLACKS_TEST_DATABASE")
            .ok_or_else(|| anyhow::anyhow!("missing test database"))?,
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let store =
                ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
            let mut transaction = store.write().await?;
            let key = support::qso_key(support::id(10));
            save::save_in(
                &mut transaction,
                key,
                None,
                support::contact(support::qso(support::id(10))),
            )
            .await?;
            std::fs::write(file.with_extension("ready"), b"uncommitted")?;
            std::future::pending::<()>().await;
            transaction.commit().await?;
            Ok::<_, anyhow::Error>(())
        })
}

#[test]
fn process_death_rolls_back_uncommitted_values() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let mut child = process::InterruptedChild::start(
        "failure_tests::transaction_child",
        &file,
        "before_commit",
    )?;
    child.wait_ready(&file.with_extension("ready"))?;
    child.kill()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
            assert!(
                save::load(&store, support::qso_key(support::id(10)))
                    .await?
                    .is_none()
            );
            assert!(matches!(
                save::save(
                    &store,
                    support::qso_key(support::id(10)),
                    None,
                    support::contact(support::qso(support::id(10)))
                )
                .await?,
                SaveOutcome::Saved(_)
            ));
            store.close().await?;
            Ok::<_, anyhow::Error>(())
        })?;
    Ok(())
}

#[test]
fn sqlite_full_rolls_back_and_counts_only_the_successful_retry() -> anyhow::Result<()> {
    let captured = telemetry::capture(|| -> anyhow::Result<()> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(full_write_and_recovery())
    });
    captured.counter(
        "clacks_client_qso_mutations_total",
        &[("change", "create")],
        1,
    );
    captured.result
}

#[test]
fn staged_input_counts_new_commits_without_counting_retries() -> anyhow::Result<()> {
    use crate::transfer;
    use clacks_sync::{
        account::{AccountMetadata, InstalledCheckpoint},
        lineage::Coverage,
        service::{VersionReference, operation::CoupledRequirement},
        transfer::{RecordRequirement, WindowManifest, WindowMode},
    };
    let captured = telemetry::capture(|| -> anyhow::Result<()> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let directory = tempfile::tempdir()?;
                let store = ClientStore::create(
                    &directory.path().join("workspace.sqlite"),
                    support::id(1),
                    StoreSettings::default(),
                )
                .await?;
                let (_, baseline) =
                    AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
                let first = support::accepted(1, support::contact(support::qso(support::id(10))));
                let second = support::accepted(2, support::contact(support::qso(support::id(11))));
                let source = InstalledCheckpoint {
                    account: support::id(1),
                    baseline: baseline.id,
                    coverage: Coverage::from(second.stamp.accepted_at),
                };
                let manifest = WindowManifest::new(
                    source.account,
                    source.clone(),
                    WindowMode::Full,
                    [&first, &second]
                        .into_iter()
                        .map(|row| RecordRequirement {
                            key: row.key,
                            stamp: row.stamp.clone(),
                            deleted: false,
                        })
                        .collect(),
                    vec![],
                    vec![baseline],
                    vec![CoupledRequirement {
                        id: support::id(90),
                        members: [&first, &second]
                            .into_iter()
                            .map(VersionReference::from)
                            .collect(),
                    }],
                )?;
                transfer::remember(&store, &manifest).await?;
                let staged = transfer::install(
                    &store,
                    &source,
                    &WindowMode::Full,
                    std::slice::from_ref(&first),
                )
                .await?;
                assert_eq!(staged.staged, vec![first.clone()]);
                for input in [std::slice::from_ref(&first), &[]] {
                    let retry =
                        transfer::install(&store, &source, &WindowMode::Full, input).await?;
                    assert_eq!(retry.staged, vec![first.clone()]);
                }
                let complete =
                    transfer::install(&store, &source, &WindowMode::Full, &[second]).await?;
                assert!(complete.staged.is_empty());
                assert_eq!(complete.accepted.len(), 2);
                transfer::finish(&store, &source, &WindowMode::Full).await?;
                store.close().await?;
                Ok(())
            })
    });
    captured
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
    captured.counter("clacks_client_sync_records_staged_total", &[], 1);
    captured.counter("clacks_client_sync_records_installed_total", &[], 2);
    Ok(())
}

#[tokio::test]
async fn record_reads_reject_native_values_not_enforced_by_relational_constraints()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    let key = support::qso_key(support::id(10));
    save::save(
        &store,
        key,
        None,
        support::contact(support::qso(support::id(10))),
    )
    .await?;
    sqlx::query_file!("tests/sql/invalid_qso_comment.sql")
        .execute(&store.pool)
        .await?;
    assert!(save::load(&store, key).await.is_err());
    store.close().await?;
    Ok(())
}
