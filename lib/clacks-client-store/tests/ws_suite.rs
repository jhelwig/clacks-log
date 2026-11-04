use crate::support::id;
use clacks_client_store::{
    store::{ClientStore, StoreSettings},
    ws_suite::{self, ReceiverConnection, ReceiverSettings},
};

#[tokio::test]
async fn receiver_preferences_reopen() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&path, id(1), StoreSettings::default()).await?;
    assert_eq!(ws_suite::load(&store).await?, ReceiverSettings::default());
    let next = ReceiverSettings {
        enabled: true,
        connection: ReceiverConnection::Unicast {
            address: "127.0.0.1".parse()?,
            port: 2238,
        },
    };
    assert!(ws_suite::save(&store, &next).await?.changed);
    store.close().await?;

    let store = ClientStore::open(&path, id(1), StoreSettings::default()).await?;
    assert_eq!(ws_suite::load(&store).await?, next);
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn receiver_preferences_noop() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let next = ReceiverSettings::default();
    assert!(!ws_suite::save(&store, &next).await?.changed);
    let mut invalid = next.clone();
    invalid.connection = ReceiverConnection::Unicast {
        address: "239.1.1.1".parse()?,
        port: 2237,
    };
    invalid.enabled = true;
    assert!(ws_suite::save(&store, &invalid).await.is_err());
    assert_eq!(ws_suite::load(&store).await?, next);
    let enabled = ReceiverSettings {
        enabled: true,
        connection: ReceiverConnection::Unicast {
            address: "127.0.0.1".parse()?,
            port: 2238,
        },
    };
    assert!(ws_suite::save(&store, &enabled).await?.changed);
    assert!(!ws_suite::save(&store, &enabled).await?.changed);
    assert_eq!(ws_suite::load(&store).await?, enabled);
    store.close().await?;
    Ok(())
}
