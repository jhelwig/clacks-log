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
        EnrollmentCommand::Resume {
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
    assert!(matches!(
        outcome,
        EnrollmentStatus::Failed {
            resume_available: false,
            ..
        }
    ));
    store.close().await?;
    Ok(())
}
