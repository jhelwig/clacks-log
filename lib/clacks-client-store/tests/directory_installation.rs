//! A workspace's native locator survives reopening and remains independent of its account peers.

use crate::support::id;
use clacks_client_store::{directory::installation, store::ClientStore};

#[tokio::test]
async fn directory_entry_is_stable_and_workspace_local() -> anyhow::Result<()> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("workspace.sqlite");
    let store = ClientStore::create(&path, id(1), Default::default()).await?;
    let reference = installation::reference(&store).await?;
    assert_eq!(installation::reference(&store).await?, reference);
    store.close().await?;
    let reopened = ClientStore::open(&path, id(1), Default::default()).await?;
    assert_eq!(installation::reference(&reopened).await?, reference);
    let other = ClientStore::create(
        &temporary.path().join("other.sqlite"),
        id(1),
        Default::default(),
    )
    .await?;
    assert_ne!(
        installation::reference(&other).await?,
        reference,
        "same-account workspaces must not overwrite each other's native installation"
    );
    other.close().await?;
    reopened.close().await?;
    Ok(())
}
