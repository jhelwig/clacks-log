use clacks_client_store::{
    error::StoreError,
    identity::{self, EnrollmentBinding},
    store::{ClientStore, StoreSettings},
};
use clacks_model::ids::AccountId;
use tempfile::tempdir;

fn account(n: u128) -> AccountId {
    ulid::Ulid::from(n).to_string().parse().unwrap()
}

#[tokio::test]
async fn create_never_overwrites() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, account(1), StoreSettings::default()).await?;
    assert!(!identity::status(&store).await?.enrolled);
    store.close().await?;
    let before = std::fs::read(&file)?;
    assert!(
        ClientStore::create(&file, account(1), StoreSettings::default())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[tokio::test]
async fn open_rejects_wrong_account_or_unrelated_file() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, account(1), StoreSettings::default()).await?;
    store.close().await?;
    let before = std::fs::read(&file)?;
    assert!(
        ClientStore::open(&file, account(2), StoreSettings::default())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&file)?, before);
    let unrelated = dir.path().join("other.sqlite");
    std::fs::write(&unrelated, b"not a database")?;
    assert!(
        ClientStore::open(&unrelated, account(1), StoreSettings::default())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&unrelated)?, b"not a database");
    assert!(
        ClientStore::open(
            &dir.path().join("missing"),
            account(1),
            StoreSettings::default()
        )
        .await
        .is_err()
    );
    assert!(!dir.path().join("missing").exists());
    Ok(())
}

#[tokio::test]
async fn binding_checks_account_and_prior_binding() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, account(1), StoreSettings::default()).await?;
    let binding = EnrollmentBinding {
        account: account(1),
        server: "https://sync.example".into(),
        actor: ulid::Ulid::from(3_u128).to_string().parse()?,
        credential_reference: "protected:entry-1".into(),
    };
    identity::bind(&store, None, binding.clone()).await?;
    assert!(identity::status(&store).await?.enrolled);
    let error = identity::bind(&store, None, binding.clone())
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StoreError>(),
        Some(StoreError::StaleBinding)
    ));
    let mut next = binding.clone();
    next.actor = ulid::Ulid::from(4_u128).to_string().parse()?;
    identity::bind(&store, Some(&binding), next.clone()).await?;
    let mut foreign = next.clone();
    foreign.account = account(2);
    let error = identity::bind(&store, Some(&next), foreign)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StoreError>(),
        Some(StoreError::WrongAccount { .. })
    ));
    store.close().await?;
    let reopened = ClientStore::open(&file, account(1), StoreSettings::default()).await?;
    assert!(identity::status(&reopened).await?.enrolled);
    // The failed updates must not replace the last committed binding.
    identity::bind(&reopened, Some(&next), next.clone()).await?;
    reopened.close().await?;
    Ok(())
}
