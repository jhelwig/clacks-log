//! Enrollment completion stays private to the worker until the session joins it.

use super::execute;
use crate::enrollment::{EnrollmentCommand, EnrollmentStatus};
use crate::workspace::WorkspacePaths;
use clacks_client_store::store::ClientStore;
use clacks_model::ids::AccountId;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn worker_leaves_terminal_publication_to_session_admission() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = WorkspacePaths::at(directory.path())?;
    let store = ClientStore::create(&paths.database, AccountId::new(), Default::default()).await?;
    let (updates, progress) = watch::channel(EnrollmentStatus::default());

    let outcome = execute(
        store.clone(),
        paths,
        EnrollmentCommand::Connect {
            server: "invalid".into(),
            label: "Desktop".into(),
        },
        updates,
        CancellationToken::new(),
    )
    .await;

    assert!(
        matches!(*progress.borrow(), EnrollmentStatus::Saving),
        "the worker enabled retry before the session removed its task"
    );
    assert!(matches!(outcome, EnrollmentStatus::Failed { .. }));
    store.close().await?;
    Ok(())
}

#[test]
fn invalid_connection_counts_failure_without_claiming_completion() -> anyhow::Result<()> {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || -> anyhow::Result<()> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let directory = tempfile::tempdir()?;
                let paths = WorkspacePaths::at(directory.path())?;
                let store =
                    ClientStore::create(&paths.database, AccountId::new(), Default::default())
                        .await?;
                let (updates, _) = watch::channel(EnrollmentStatus::default());
                let outcome = execute(
                    store.clone(),
                    paths,
                    EnrollmentCommand::Connect {
                        server: "PRIVATE_INPUT".into(),
                        label: "PRIVATE_LABEL".into(),
                    },
                    updates,
                    CancellationToken::new(),
                )
                .await;
                assert!(matches!(outcome, EnrollmentStatus::Failed { .. }));
                store.close().await?;
                anyhow::Ok(())
            })
    })?;
    let metrics = snapshotter.snapshot().into_vec();
    let operations = metrics
        .iter()
        .filter(|(key, _, _, _)| key.key().name() == "clacks_desktop_enrollment_operations_total")
        .collect::<Vec<_>>();
    assert_eq!(operations.len(), 1);
    let (key, _, _, count) = operations.first().unwrap();
    assert_eq!(
        key.key()
            .labels()
            .map(|label| (label.key(), label.value()))
            .collect::<Vec<_>>(),
        vec![("outcome", "failed")]
    );
    assert!(matches!(count, DebugValue::Counter(1)));
    Ok(())
}
