//! Expiration cleanup preserves context required by retained encrypted receipts.

use crate::{
    auth::oidc::VerifiedIdentity,
    enrollment::ApprovalPurpose,
    maintenance::keys::{
        account::{self, AccountRotationRequest},
        report::MaintenanceEnd,
    },
    oauth::{config, store::OAuthStore},
    secrets::keys::WrappingKeys,
    store::AccountStore,
};
use anyhow::Context;
use huskarl_core::dpop::DPoP;
use huskarl_crypto_native::asymmetric::signer::{GenerateAlgorithm, PrivateKey};
use oauth_as::store::Storage;
use sqlx::{Connection, PgConnection, postgres::PgPoolOptions};
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn expired_approval_cleanup_keeps_retained_receipts_usable() -> anyhow::Result<()> {
    let _maintenance = super::MAINTENANCE_TEST.lock().await;
    let url = std::env::var("CLACKS_TEST_DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await?;
    crate::maintenance::migrate::run(&pool).await?;
    let accounts = AccountStore::new(pool.clone());
    let login = accounts
        .provision_login(
            VerifiedIdentity {
                issuer: "https://cleanup-provider.example".into(),
                subject: ulid::Ulid::generate().to_string(),
                display_name: None,
            },
            time::Duration::days(14),
        )
        .await?;
    let principal = accounts
        .authenticate(&login, time::Duration::days(14))
        .await?
        .context("missing login")?;
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("key");
    std::fs::write(&file, [51u8; 32])?;
    let wrapping = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "cleanup".into(),
        files: [("cleanup".into(), file)].into(),
    })?);
    let store = OAuthStore::new(
        crate::store::AccountStore::new(pool.clone()),
        wrapping.clone(),
    );
    let issuer = config::issuer(store.clone(), &"https://clacks.example".parse()?);
    let dpop = DPoP::builder()
        .signer(PrivateKey::generate(GenerateAlgorithm::Es256, None)?)
        .build();
    let now = SystemTime::now();
    let issued = super::super::enrollment::approve(
        &issuer,
        &store,
        &principal,
        &dpop,
        "Desktop",
        ApprovalPurpose::New,
    )
    .await?;
    // Exercise Clacks's approval-collection query at the boundary where a
    // bounded code-collection batch has left a stored receipt behind.
    let mut tx = pool.begin().await?;
    crate::store::tests::support::lock(&mut tx, principal.account_id).await?;
    let expired_at = time::OffsetDateTime::from(now + Duration::from_secs(1200));
    sqlx::query_file!(
        "sql/oauth/cleanup_desktop_code_approvals.sql",
        principal.account_id.to_string(),
        expired_at
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let request = AccountRotationRequest::new(principal.account_id, 1, 2, 100)?;
    let result = account::rotate(
        PgConnection::connect(&url).await?,
        &wrapping,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(result.end, MaintenanceEnd::Complete);
    assert_eq!(result.migrated_receipts, 1);
    // Migration must leave the consumed code decodable under its immutable context.
    assert!(store.take_authorization_code(&issued.code).await?.is_some());
    pool.close().await;
    Ok(())
}
