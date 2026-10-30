//! Credential ownership and stale-write behavior through production transactions.

use crate::{
    auth::oidc::VerifiedIdentity,
    secrets::{
        credentials::{self, CredentialIdentity, CredentialPurpose, SecretMaterial},
        keys::WrappingKeys,
    },
    store::AccountStore,
};
use anyhow::Context;
use clacks_model::ids::{AccountId, CredentialId};
use sqlx::{PgConnection, PgPool, postgres::PgPoolOptions};
use time::Duration;
use zeroize::Zeroizing;

pub(super) async fn account(pool: &PgPool) -> anyhow::Result<AccountId> {
    let store = AccountStore::new(pool.clone());
    let login = store
        .provision_login(
            VerifiedIdentity {
                issuer: "https://credential-provider.example".into(),
                subject: ulid::Ulid::generate().to_string(),
                display_name: None,
            },
            Duration::days(14),
        )
        .await?;
    Ok(store
        .authenticate(&login, Duration::days(14))
        .await?
        .context("missing new account")?
        .account_id)
}

pub(super) async fn lock(connection: &mut PgConnection, account: AccountId) -> anyhow::Result<()> {
    sqlx::query_file!("sql/account_lock.sql", account.to_string())
        .fetch_one(connection)
        .await?;
    Ok(())
}

pub(super) fn material(value: &[u8]) -> SecretMaterial {
    SecretMaterial {
        payload_format: 1,
        bytes: Zeroizing::new(value.to_vec()),
    }
}

#[tokio::test]
async fn credential_changes_preserve_account_purpose_and_removal() -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?)
        .await?;
    crate::maintenance::migrate::run(&pool).await?;
    pool.close().await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?)
        .await?;
    let owner = account(&pool).await?;
    let other = account(&pool).await?;
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("key");
    std::fs::write(&file, [19u8; 32])?;
    let wrapping = WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "W1".into(),
        files: [("W1".into(), file)].into(),
    })?;
    let identity = CredentialIdentity {
        account: owner,
        credential: CredentialId::new(),
        purpose: CredentialPurpose::QrzDirectory,
    };

    let mut tx = pool.begin().await?;
    lock(&mut tx, owner).await?;
    let first = credentials::put_in(
        &mut tx,
        &wrapping,
        identity,
        None,
        material(b"first-provider-secret"),
    )
    .await?;
    assert_eq!(first.material_revision, 1);
    tx.commit().await?;

    let mut tx = pool.begin().await?;
    let saved = credentials::read_in(&mut tx, &wrapping, identity)
        .await?
        .context("missing stored credential")?;
    assert_eq!(
        saved.material.context("missing material")?.bytes.as_slice(),
        b"first-provider-secret"
    );
    assert!(
        credentials::read_in(
            &mut tx,
            &wrapping,
            CredentialIdentity {
                account: other,
                ..identity
            }
        )
        .await?
        .is_none()
    );
    assert!(
        credentials::read_in(
            &mut tx,
            &wrapping,
            CredentialIdentity {
                purpose: CredentialPurpose::LotwSigning,
                ..identity
            }
        )
        .await
        .is_err()
    );
    tx.commit().await?;

    let mut tx = pool.begin().await?;
    lock(&mut tx, owner).await?;
    let replaced = credentials::put_in(
        &mut tx,
        &wrapping,
        identity,
        Some(1),
        material(b"replacement-secret"),
    )
    .await?;
    assert_eq!(replaced.material_revision, 2);
    let removed = credentials::remove_in(&mut tx, identity, 2).await?;
    assert_eq!(removed.material_revision, 3);
    tx.commit().await?;

    let mut tx = pool.begin().await?;
    lock(&mut tx, owner).await?;
    assert!(
        credentials::put_in(
            &mut tx,
            &wrapping,
            identity,
            Some(2),
            material(b"stale-secret")
        )
        .await
        .is_err()
    );
    let saved = credentials::read_in(&mut tx, &wrapping, identity)
        .await?
        .context("missing removal")?;
    assert!(saved.material.is_none());
    assert_eq!(saved.metadata.material_revision, 3);
    let restored = credentials::put_in(
        &mut tx,
        &wrapping,
        identity,
        Some(3),
        material(b"explicit-new-secret"),
    )
    .await?;
    assert_eq!(restored.material_revision, 4);
    assert_eq!(restored.created_at, first.created_at);
    tx.commit().await?;
    pool.close().await;
    Ok(())
}
