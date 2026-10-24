//! Store operations must be spawnable on the desktop's multi-thread runtime.

use clacks_client_store::{
    identity, operating,
    store::{ClientStore, StoreSettings},
};
use clacks_model::{defaults::account_definitions, ids::AccountId};

fn requires_send<T: Send>(_: T) {}

#[tokio::test]
async fn workspace_futures_are_send() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("workspace.sqlite");
    let definitions = account_definitions()?;
    requires_send(ClientStore::create_with_definitions(
        &path,
        AccountId::new(),
        &definitions,
        StoreSettings::default(),
    ));
    requires_send(ClientStore::open_discovered(
        &path,
        StoreSettings::default(),
    ));
    let store = ClientStore::create(&path, AccountId::new(), StoreSettings::default()).await?;
    requires_send(identity::status(&store));
    requires_send(operating::load(&store));
    store.close().await?;
    Ok(())
}
