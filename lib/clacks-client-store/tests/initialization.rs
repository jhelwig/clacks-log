use clacks_client_store::{
    identity, save,
    store::{ClientStore, StoreSettings},
};
use clacks_model::{
    defaults::account_definitions,
    ids::{AccountId, ClientActorId},
};
use clacks_sync::record::{Observation, RecordId, RecordKey, RecordState, RecordValue};

#[tokio::test]
async fn discover_existing_account_once() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("workspace.sqlite");
    let account = AccountId::new();
    ClientStore::create(&path, account, StoreSettings::default())
        .await?
        .close()
        .await?;
    let store = ClientStore::open_discovered(&path, StoreSettings::default()).await?;
    assert_eq!(store.account_id(), account);
    assert_eq!(
        identity::status(&store).await?,
        identity::WorkspaceStatus {
            account,
            enrolled: false
        }
    );
    store.close().await
}

#[tokio::test]
async fn expected_account_check_is_preserved() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("workspace.sqlite");
    ClientStore::create(&path, AccountId::new(), StoreSettings::default())
        .await?
        .close()
        .await?;
    assert!(
        ClientStore::open(&path, AccountId::new(), StoreSettings::default())
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn create_account_and_definitions_atomically() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("workspace.sqlite");
    let account = AccountId::new();
    let definitions = account_definitions()?;
    let store = ClientStore::create_with_definitions(
        &path,
        account,
        &definitions,
        StoreSettings::default(),
    )
    .await?;
    for group in &definitions.groups {
        let key = RecordKey {
            account,
            id: RecordId::Group(group.id),
        };
        let record = save::load(&store, key).await?.unwrap();
        assert!(matches!(record.latest, Observation::Unavailable { .. }));
        assert!(
            record
                .pending
                .as_ref()
                .is_some_and(|pending| pending.prepared.is_none())
        );
        assert_eq!(
            record.working_state(),
            Some(&RecordState::Live(RecordValue::Group(Box::new(
                group.clone()
            ))))
        );
    }
    store.close().await
}

#[tokio::test]
async fn failed_seed_install_leaves_no_account() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("workspace.sqlite");
    let mut definitions = account_definitions()?;
    definitions.output_profiles.clear();
    assert!(
        ClientStore::create_with_definitions(
            &path,
            AccountId::new(),
            &definitions,
            StoreSettings::default()
        )
        .await
        .is_err()
    );
    assert!(
        !path.exists(),
        "invalid input must be rejected before creating a file"
    );
    Ok(())
}

#[tokio::test]
async fn reopen_preserves_changed_definitions() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("workspace.sqlite");
    let definitions = account_definitions()?;
    let group = definitions.groups.first().unwrap();
    let account = AccountId::new();
    let store = ClientStore::create_with_definitions(
        &path,
        account,
        &definitions,
        StoreSettings::default(),
    )
    .await?;
    let key = RecordKey {
        account,
        id: RecordId::Group(group.id),
    };
    let expected = save::load(&store, key).await?;
    let mut changed = group.clone();
    changed.name = "Local custom name".into();
    let value = RecordState::Live(RecordValue::Group(Box::new(changed)));
    save::save(&store, key, expected.as_ref(), value.clone()).await?;
    store.close().await?;
    let reopened = ClientStore::open_discovered(&path, StoreSettings::default()).await?;
    assert_eq!(
        save::load(&reopened, key).await?.unwrap().working_state(),
        Some(&value)
    );
    reopened.close().await
}

#[tokio::test]
async fn seed_creation_fabricates_no_shared_evidence() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let account = AccountId::new();
    let store = ClientStore::create_with_definitions(
        &directory.path().join("workspace.sqlite"),
        account,
        &account_definitions()?,
        StoreSettings::default(),
    )
    .await?;
    let mut snapshot = clacks_client_store::read::snapshot(&store).await?;
    assert!(snapshot.checkpoint().await?.is_none());
    assert!(snapshot.account_view().await?.is_none());
    snapshot.close().await?;
    let binding = identity::EnrollmentBinding {
        account,
        server: "https://example.invalid".into(),
        actor: ClientActorId::new(),
        credential_reference: "protected-reference".into(),
    };
    assert!(identity::bind(&store, None, binding).await.is_err());
    store.close().await
}

#[tokio::test]
async fn status_distinguishes_enrollment_without_exposing_credentials() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let account = AccountId::new();
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        account,
        StoreSettings::default(),
    )
    .await?;
    identity::bind(
        &store,
        None,
        identity::EnrollmentBinding {
            account,
            server: "https://example.invalid".into(),
            actor: ClientActorId::new(),
            credential_reference: "secret-reference-sentinel".into(),
        },
    )
    .await?;
    let status = identity::status(&store).await?;
    assert!(status.enrolled);
    assert_eq!(status.account, account);
    assert!(!format!("{status:?}").contains("secret-reference-sentinel"));
    store.close().await
}
