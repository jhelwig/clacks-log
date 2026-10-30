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
async fn created_definitions_survive_reopen_as_unpublished_values() -> anyhow::Result<()> {
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
    store.close().await?;
    let store = ClientStore::open_discovered(&path, StoreSettings::default()).await?;
    assert_eq!(store.account_id(), account);
    assert_eq!(
        identity::status(&store).await?,
        identity::WorkspaceStatus {
            account,
            enrolled: false
        }
    );
    let mut snapshot = clacks_client_store::read::snapshot(&store).await?;
    assert!(snapshot.checkpoint().await?.is_none());
    assert!(snapshot.account_view().await?.is_none());
    snapshot.close().await?;

    let error = identity::bind(
        &store,
        None,
        identity::EnrollmentBinding {
            account,
            server: "https://example.invalid".into(),
            actor: ClientActorId::new(),
            credential_reference: "protected-reference".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<clacks_client_store::error::StoreError>(),
        Some(clacks_client_store::error::StoreError::PopulatedWorkspace)
    ));

    for group in &definitions.groups {
        let key = RecordKey {
            account,
            id: RecordId::Group(group.id),
        };
        let record = save::load(&store, key).await?.unwrap();

        assert!(matches!(record.latest(), Observation::Unavailable { .. }));
        assert!(
            record
                .pending()
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

    for profile in &definitions.output_profiles {
        let record = save::load(
            &store,
            RecordKey {
                account,
                id: RecordId::OutputProfile(profile.id),
            },
        )
        .await?
        .unwrap();
        assert!(matches!(record.latest(), Observation::Unavailable { .. }));
        assert!(
            record
                .pending()
                .as_ref()
                .is_some_and(|pending| pending.prepared.is_none())
        );
        assert_eq!(
            record.working_state(),
            Some(&RecordState::Live(RecordValue::OutputProfile(Box::new(
                profile.clone()
            ))))
        );
    }
    store.close().await
}

#[tokio::test]
async fn invalid_seed_is_rejected_before_file_creation() -> anyhow::Result<()> {
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
