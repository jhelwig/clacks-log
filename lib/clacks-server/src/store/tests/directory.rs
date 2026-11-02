//! Directory lifecycle assertions through the production account store.

use crate::{
    auth::oidc::VerifiedIdentity,
    config::WrappingKeyConfig,
    directory::{CredentialChange, DirectoryUpdate, DirectoryUpdateOutcome},
    secrets::keys::WrappingKeys,
    store::{AccountStore, directory},
};
use clacks_model::directory::values::DirectoryProvider;
use clacks_sync::protocol::directory::DirectoryLogin;
use sqlx::postgres::PgPoolOptions;
use time::Duration;

#[tokio::test]
async fn directory_configuration_is_account_scoped_and_stale_safe() -> anyhow::Result<()> {
    let _maintenance = crate::maintenance::keys::tests::SERIAL.lock().await;
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?)
        .await?;
    crate::maintenance::migrate::run(&pool).await?;
    let store = AccountStore::new(pool.clone());
    let mut principals = Vec::new();
    for _ in 0..2 {
        let login = store
            .provision_login(
                VerifiedIdentity {
                    issuer: "https://directory-test.example".into(),
                    subject: ulid::Ulid::generate().to_string(),
                    display_name: None,
                },
                Duration::days(1),
            )
            .await?;
        principals.push(
            store
                .authenticate(&login, Duration::days(1))
                .await?
                .unwrap(),
        );
    }
    let first = principals.first().unwrap();
    let second = principals.last().unwrap();
    let temporary = tempfile::tempdir()?;
    let keyfile = temporary.path().join("key");
    std::fs::write(&keyfile, [27u8; 32])?;
    let wrapping_id = format!("{}-directory-test", first.account_id);
    let keys = WrappingKeys::load(&WrappingKeyConfig {
        active: wrapping_id.clone(),
        files: [(wrapping_id.clone(), keyfile.clone())].into(),
    })?;
    let changed = directory::update(
        &store,
        &keys,
        first,
        DirectoryUpdate {
            expected_revision: 0,
            selected: Some(DirectoryProvider::Qrz),
            credential: CredentialChange::Replace {
                provider: DirectoryProvider::Qrz,
                login: DirectoryLogin {
                    username: "fixture-user".into(),
                    password: "directory-test-secret".into(),
                },
            },
        },
    )
    .await?;
    let DirectoryUpdateOutcome::Committed(configuration) = changed else {
        anyhow::bail!("replacement not committed");
    };
    assert_eq!(configuration.revision, 1);
    assert!(configuration.configured.contains(&DirectoryProvider::Qrz));
    assert_eq!(directory::load(&store, second).await?.revision, 0);
    let stale = directory::update(
        &store,
        &keys,
        first,
        DirectoryUpdate {
            expected_revision: 0,
            selected: None,
            credential: CredentialChange::Keep,
        },
    )
    .await?;
    assert!(matches!(stale, DirectoryUpdateOutcome::Stale(c) if c.revision == 1));
    let unchanged = directory::update(
        &store,
        &keys,
        first,
        DirectoryUpdate {
            expected_revision: 1,
            selected: Some(DirectoryProvider::Qrz),
            credential: CredentialChange::Keep,
        },
    )
    .await?;
    assert!(matches!(unchanged, DirectoryUpdateOutcome::Unchanged(c) if c.revision == 1));
    let old = directory::material(
        &mut *pool.acquire().await?,
        &keys,
        first.account_id,
        DirectoryProvider::Qrz,
    )
    .await?
    .unwrap();
    directory::update(
        &store,
        &keys,
        first,
        DirectoryUpdate {
            expected_revision: 1,
            selected: Some(DirectoryProvider::Qrz),
            credential: CredentialChange::Replace {
                provider: DirectoryProvider::Qrz,
                login: DirectoryLogin {
                    username: "fixture-user".into(),
                    password: "replacement-secret".into(),
                },
            },
        },
    )
    .await?;
    let current = directory::material(
        &mut *pool.acquire().await?,
        &keys,
        first.account_id,
        DirectoryProvider::Qrz,
    )
    .await?
    .unwrap();
    assert_eq!(current.id, old.id);
    assert_eq!(current.revision, 2);
    assert_eq!(current.login.password, "replacement-secret");
    use crate::maintenance::keys::account::{AccountRotationRequest, rotate};
    use sqlx::Connection;
    let request = AccountRotationRequest::new(first.account_id, 1, 2, 1)?;
    let report = rotate(
        sqlx::PgConnection::connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?).await?,
        &keys,
        &request,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await?;
    assert_eq!(
        report.migrated_values, 1,
        "rotation must include actual directory material"
    );
    assert_eq!(report.remaining_values, 0);
    let restored = directory::material(
        &mut *pool.acquire().await?,
        &keys,
        first.account_id,
        DirectoryProvider::Qrz,
    )
    .await?
    .unwrap();
    assert_eq!(restored, current);
    let repeated = rotate(
        sqlx::PgConnection::connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?).await?,
        &keys,
        &request,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await?;
    assert_eq!(repeated.migrated_values, 0);
    assert_eq!(directory::load(&store, first).await?.revision, 2);
    let target_file = temporary.path().join("target-key");
    std::fs::write(&target_file, [28u8; 32])?;
    let target_id = format!("{}-next", wrapping_id);
    let target = WrappingKeys::load(&WrappingKeyConfig {
        active: target_id.clone(),
        files: [
            (wrapping_id.clone(), keyfile),
            (target_id.clone(), target_file.clone()),
        ]
        .into(),
    })?;
    let request =
        crate::maintenance::keys::rewrap::RewrapRequest::new(wrapping_id, target_id.clone(), 1)?;
    let report = crate::maintenance::keys::rewrap::run(
        sqlx::PgConnection::connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?).await?,
        &target,
        &request,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await?;
    assert_eq!(report.migrated_account_keys, 2);
    let keys = WrappingKeys::load(&WrappingKeyConfig {
        active: target_id.clone(),
        files: [(target_id, target_file)].into(),
    })?;
    assert_eq!(
        directory::material(
            &mut *pool.acquire().await?,
            &keys,
            first.account_id,
            DirectoryProvider::Qrz
        )
        .await?
        .unwrap(),
        current
    );

    directory::update(
        &store,
        &keys,
        first,
        DirectoryUpdate {
            expected_revision: 2,
            selected: Some(DirectoryProvider::Qrz),
            credential: CredentialChange::Remove {
                provider: DirectoryProvider::Qrz,
            },
        },
    )
    .await?;
    let removed = directory::load(&store, first).await?;
    assert_eq!(removed.revision, 3);
    assert_eq!(removed.selected, Some(DirectoryProvider::Qrz));
    assert!(removed.configured.is_empty());
    assert!(
        directory::material(
            &mut *pool.acquire().await?,
            &keys,
            first.account_id,
            DirectoryProvider::Qrz
        )
        .await?
        .is_none()
    );
    let unchanged = directory::update(
        &store,
        &keys,
        first,
        DirectoryUpdate {
            expected_revision: 3,
            selected: Some(DirectoryProvider::Qrz),
            credential: CredentialChange::Remove {
                provider: DirectoryProvider::Qrz,
            },
        },
    )
    .await?;
    assert!(matches!(unchanged, DirectoryUpdateOutcome::Unchanged(c) if c.revision == 3));

    // Credential forms preserve selection and permit either field to change alone.
    for (revision, username, password) in [
        (3, "saved-user", Some("first-password")),
        (4, "renamed-user", None),
        (5, "renamed-user", Some("new-password")),
    ] {
        let outcome = directory::update_preserving_selection(
            &store,
            &keys,
            first,
            DirectoryUpdate {
                expected_revision: revision,
                selected: None,
                credential: CredentialChange::Save {
                    provider: DirectoryProvider::Qrz,
                    username: username.into(),
                    password: password.map(str::to_owned),
                },
            },
        )
        .await?;
        assert!(
            matches!(outcome, DirectoryUpdateOutcome::Committed(c) if c.selected == Some(DirectoryProvider::Qrz))
        );
        let saved = directory::material(
            &mut *pool.acquire().await?,
            &keys,
            first.account_id,
            DirectoryProvider::Qrz,
        )
        .await?
        .unwrap();
        assert_eq!(saved.login.username, username);
        assert_eq!(
            saved.login.password,
            if revision < 5 {
                "first-password"
            } else {
                "new-password"
            }
        );
        let setup = directory::setup(&store, &keys, first).await?;
        assert_eq!(
            setup
                .usernames
                .get(&DirectoryProvider::Qrz)
                .map(String::as_str),
            Some(username)
        );
    }
    assert!(
        directory::setup(&store, &keys, second)
            .await?
            .usernames
            .is_empty()
    );
    Ok(())
}
