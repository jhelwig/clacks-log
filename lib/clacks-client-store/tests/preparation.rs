use crate::support;
use clacks_client_store::{
    save,
    store::{ClientStore, StoreSettings},
    synchronization::{self, PrepareOutcome},
};

#[tokio::test]
async fn unenrolled_cannot_prepare_or_allocate_an_author() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
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
    assert!(matches!(
        synchronization::prepare(&store, key).await?,
        PrepareOutcome::NotEnrolled
    ));
    assert!(
        save::load(&store, key)
            .await?
            .unwrap()
            .pending
            .unwrap()
            .prepared
            .is_none()
    );
    store.close().await?;
    Ok(())
}
