//! Key maintenance preserves real issuer receipts and authorization outcomes.

mod rewrap;

use crate::{
    auth::oidc::VerifiedIdentity,
    enrollment::ApprovalPurpose,
    maintenance::keys::{
        account::{self as rotation, AccountRotationRequest},
        migration,
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
use std::sync::Arc;
use time::Duration;
use tokio_util::sync::CancellationToken;

use crate::maintenance::keys::tests::SERIAL as MAINTENANCE_TEST;

#[tokio::test]
async fn account_rotation_resumes_and_preserves_enrollment() -> anyhow::Result<()> {
    let _maintenance = MAINTENANCE_TEST.lock().await;
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshot = recorder.snapshotter();
    let _metrics_guard = metrics::set_default_local_recorder(&recorder);
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .with_writer(output.reopen()?)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
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
                issuer: "https://rotation-provider.example".into(),
                subject: ulid::Ulid::generate().to_string(),
                display_name: None,
            },
            Duration::days(14),
        )
        .await?;
    let principal = accounts
        .authenticate(&login, Duration::days(14))
        .await?
        .context("missing login")?;
    let owner = principal.account_id;
    let mut status_connection = pool.acquire().await?;
    let keyless =
        crate::maintenance::keys::references::status(&mut status_connection, Some(owner)).await?;
    let crate::maintenance::keys::references::KeyStatus::Account { versions, .. } = keyless else {
        anyhow::bail!("wrong status scope");
    };
    assert!(versions.is_empty());
    drop(status_connection);

    let directory = tempfile::tempdir()?;
    let file = directory.path().join("W1");
    let target_file = directory.path().join("W2");
    std::fs::write(&file, [19u8; 32])?;
    std::fs::write(&target_file, [20u8; 32])?;
    let source = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "W1".into(),
        files: [("W1".into(), file.clone())].into(),
    })?);
    let store = OAuthStore::new(crate::store::AccountStore::new(pool.clone()), source);
    let issuer = config::issuer(store.clone(), &"https://clacks.example".parse()?);
    let dpop = DPoP::builder()
        .signer(PrivateKey::generate(GenerateAlgorithm::Es256, None)?)
        .build();
    let mut issued = Vec::new();
    for label in ["First", "Second", "Third"] {
        issued.push(
            super::enrollment::approve(
                &issuer,
                &store,
                &principal,
                &dpop,
                label,
                ApprovalPurpose::New,
            )
            .await?,
        );
    }
    let wrapping = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "W2".into(),
        files: [("W1".into(), file), ("W2".into(), target_file.clone())].into(),
    })?);
    let only_target = WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "W2".into(),
        files: [("W2".into(), target_file)].into(),
    })?;
    let request = AccountRotationRequest::new(owner, 1, 2, 1)?;
    let mut connection = PgConnection::connect(&url).await?;
    rotation::cutover(&mut connection, &wrapping, &request).await?;
    assert_eq!(
        migration::batch(&mut connection, &wrapping, &request).await?,
        1
    );
    connection.close().await?;

    let missing_source = rotation::rotate(
        PgConnection::connect(&url).await?,
        &only_target,
        &request,
        &CancellationToken::new(),
    )
    .await
    .err()
    .context("missing source material was accepted")?;
    let progress = missing_source
        .downcast_ref::<crate::maintenance::keys::report::AccountRotationError>()
        .context("rotation failure lost committed progress")?;
    assert_eq!(progress.migrated_receipts, 0);
    assert!(std::error::Error::source(progress).is_some());
    let diagnostic = format!("{missing_source:#}");
    assert!(diagnostic.contains(&owner.to_string()));
    assert!(diagnostic.contains("source key version 1"));

    let stop = CancellationToken::new();
    stop.cancel();
    let interrupted = rotation::rotate(
        PgConnection::connect(&url).await?,
        &wrapping,
        &request,
        &stop,
    )
    .await?;
    assert_eq!(interrupted.end, MaintenanceEnd::Interrupted);
    assert_eq!(interrupted.migrated_receipts, 0);
    assert_eq!(interrupted.remaining_receipts, 2);
    let report = rotation::rotate(
        PgConnection::connect(&url).await?,
        &wrapping,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(report.end, MaintenanceEnd::Complete);
    assert_eq!(report.migrated_receipts, 2);
    assert_eq!(report.remaining_receipts, 0);
    let again = rotation::rotate(
        PgConnection::connect(&url).await?,
        &wrapping,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(again.migrated_receipts, 0);
    assert!(AccountRotationRequest::new(owner, 2, 4, 1).is_err());

    let current_store = OAuthStore::new(crate::store::AccountStore::new(pool.clone()), wrapping);
    for client in &issued {
        assert!(
            current_store
                .authorized(&client.response.access_token)
                .await?
                .is_some()
        );
        let retained = current_store
            .take_authorization_code(&client.code)
            .await?
            .context("missing retained receipt")?;
        current_store.put_authorization_code(retained).await?;
    }
    let events = std::fs::read_to_string(output.path())?;
    assert!(events.contains("account.key.cutover"));
    assert!(events.contains(&owner.to_string()));
    assert!(events.contains("\"to_version\":2"));
    assert!(events.contains("\"migrated_receipts\":2"));
    for client in &issued {
        assert!(!events.contains(&client.code));
        assert!(!events.contains(&client.response.access_token));
    }
    let metric = snapshot
        .snapshot()
        .into_vec()
        .into_iter()
        .find(|(key, _, _, _)| key.key().name() == "clacks_secret_values_reencrypted_total")
        .context("missing committed receipt count")?;
    assert_eq!(metric.3, metrics_util::debugging::DebugValue::Counter(3));
    assert_eq!(metric.0.key().labels().count(), 0);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn taken_receipt_is_resealed_after_rotation_and_still_contains_replay() -> anyhow::Result<()>
{
    use crate::{
        auth::oidc::VerifiedIdentity,
        enrollment::ApprovalPurpose,
        oauth::{config, store::OAuthStore},
        store::AccountStore,
    };
    use huskarl_core::dpop::DPoP;
    use huskarl_crypto_native::asymmetric::signer::{GenerateAlgorithm, PrivateKey};
    use oauth_as::store::Storage;
    use std::sync::Arc;
    use time::Duration;

    let _maintenance = MAINTENANCE_TEST.lock().await;
    let url = std::env::var("CLACKS_TEST_DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await?;
    crate::maintenance::migrate::run(&pool).await?;
    let accounts = AccountStore::new(pool.clone());
    let login = accounts
        .provision_login(
            VerifiedIdentity {
                issuer: "https://rotation-provider.example".into(),
                subject: ulid::Ulid::generate().to_string(),
                display_name: None,
            },
            Duration::days(14),
        )
        .await?;
    let principal = accounts
        .authenticate(&login, Duration::days(14))
        .await?
        .context("missing login")?;
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("key");
    std::fs::write(&file, [27u8; 32])?;
    let wrapping = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "W1".into(),
        files: [("W1".into(), file)].into(),
    })?);
    let store = OAuthStore::new(
        crate::store::AccountStore::new(pool.clone()),
        wrapping.clone(),
    );
    let issuer = config::issuer(store.clone(), &"https://clacks.example".parse()?);
    let dpop = DPoP::builder()
        .signer(PrivateKey::generate(GenerateAlgorithm::Es256, None)?)
        .build();
    let first = super::enrollment::approve(
        &issuer,
        &store,
        &principal,
        &dpop,
        "First",
        ApprovalPurpose::New,
    )
    .await?;
    let second = super::enrollment::approve(
        &issuer,
        &store,
        &principal,
        &dpop,
        "Second",
        ApprovalPurpose::New,
    )
    .await?;
    let taken = store
        .take_authorization_code(&first.code)
        .await?
        .context("missing consumed code")?;
    let request = AccountRotationRequest::new(principal.account_id, 1, 2, 1)?;
    let report = rotation::rotate(
        PgConnection::connect(&url).await?,
        &wrapping,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(report.migrated_receipts, 1);
    store.put_authorization_code(taken).await?;
    let mut connection = pool.acquire().await?;
    let counts =
        crate::maintenance::keys::references::count(&mut connection, principal.account_id, 2)
            .await?;
    assert_eq!(counts, 2);
    drop(connection);
    assert!(
        super::enrollment::exchange(
            &issuer,
            &dpop,
            &first.code,
            "http://127.0.0.1:49123/oauth/callback",
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        )
        .await
        .is_err()
    );
    assert!(
        store
            .authorized(&first.response.access_token)
            .await?
            .is_none()
    );
    assert!(
        store
            .authorized(&second.response.access_token)
            .await?
            .is_some()
    );
    pool.close().await;
    Ok(())
}
