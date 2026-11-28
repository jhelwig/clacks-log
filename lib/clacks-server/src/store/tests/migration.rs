//! Existing account-only state upgrades through production migration and native admission.

use crate::{
    auth::oidc::VerifiedIdentity,
    enrollment::ApprovalPurpose,
    maintenance::migrate,
    oauth::{config, store::OAuthStore},
    secrets::keys::WrappingKeys,
    store::{AccountStore, metadata::account},
};
use clacks_model::ids::AccountId;
use huskarl_core::dpop::DPoP;
use huskarl_crypto_native::asymmetric::signer::{GenerateAlgorithm, PrivateKey};
use jiff::SignedDuration;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[tokio::test]
async fn existing_account_can_enroll_after_database_upgrade() -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?)
        .await?;
    migrate::run(&pool).await?;
    let owner = AccountId::new();
    let identity = ulid::Ulid::generate();
    let subject = ulid::Ulid::generate().to_string();
    sqlx::query_file!(
        "src/store/tests/migration/pre_sync_account.sql",
        owner.to_string(),
        identity.to_string(),
        subject,
    )
    .execute(&pool)
    .await?;

    migrate::run(&pool).await?;
    let store = AccountStore::new(pool.clone());
    let login = store
        .provision_login(
            VerifiedIdentity {
                issuer: "https://migration-fixture.example".into(),
                subject,
                display_name: Some("Existing operator".into()),
            },
            SignedDuration::from_hours(14 * 24),
        )
        .await?;
    let principal = store
        .authenticate(&login, SignedDuration::from_hours(14 * 24))
        .await?
        .unwrap();
    assert_eq!(principal.account_id, owner);
    assert_eq!(principal.identity_id.to_string(), identity.to_string());

    let directory = tempfile::tempdir()?;
    let file = directory.path().join("wrapping-key");
    std::fs::write(&file, [31_u8; 32])?;
    let keys = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "test".into(),
        files: [("test".into(), file)].into(),
    })?);
    let oauth = OAuthStore::new(store, keys);
    let issuer = config::issuer(oauth.clone(), &"https://clacks.example".parse()?);
    let dpop = DPoP::builder()
        .signer(PrivateKey::generate(GenerateAlgorithm::Es256, None)?)
        .build();
    let issued = super::enrollment::approve(
        &issuer,
        &oauth,
        &principal,
        &dpop,
        "Existing account desktop",
        ApprovalPurpose::New,
    )
    .await?;
    assert_eq!(
        oauth
            .authorized(&issued.response.access_token)
            .await?
            .unwrap()
            .account,
        owner
    );

    let mut connection = pool.acquire().await?;
    let before = account::load(&mut connection, owner).await?;
    drop(connection);
    migrate::run(&pool).await?;
    let mut connection = pool.acquire().await?;
    assert_eq!(
        account::load(&mut connection, owner).await?,
        before,
        "repeated migration must not reset timeline, definitions or membership"
    );
    Ok(())
}
