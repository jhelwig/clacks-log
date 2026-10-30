//! Key rotation retains credential meaning and real authorization behavior.

mod cleanup;
mod rewrap;

use super::credentials::{account, lock, material};
use crate::{
    maintenance::keys::{
        account::{self as rotation, AccountRotationRequest},
        migration::{self, CiphertextFamily},
        report::MaintenanceEnd,
    },
    secrets::{
        credentials::{self, CredentialIdentity, CredentialPurpose},
        keys::WrappingKeys,
    },
};
use anyhow::Context;
use clacks_model::ids::CredentialId;
use sqlx::{Connection, PgConnection, postgres::PgPoolOptions};
use std::num::NonZeroU16;
use tokio_util::sync::CancellationToken;

use crate::maintenance::keys::tests::SERIAL as MAINTENANCE_TEST;

#[tokio::test]
async fn account_rotation_resumes_and_preserves_credential_meaning() -> anyhow::Result<()> {
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
    let owner = account(&pool).await?;
    let mut status_connection = pool.acquire().await?;
    let keyless =
        crate::maintenance::keys::references::status(&mut status_connection, Some(owner)).await?;
    let crate::maintenance::keys::references::KeyStatus::Account { versions, .. } = keyless else {
        anyhow::bail!("wrong status scope");
    };
    assert!(versions.is_empty());
    drop(status_connection);
    let directory = tempfile::tempdir()?;

    let file = directory.path().join("key");
    std::fs::write(&file, [19u8; 32])?;
    let wrapping = WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "W1".into(),
        files: [("W1".into(), file.clone())].into(),
    })?;
    let mut identities = Vec::new();
    let mut tx = pool.begin().await?;
    lock(&mut tx, owner).await?;
    for _ in 0..3 {
        let identity = CredentialIdentity {
            account: owner,
            credential: CredentialId::new(),
            purpose: CredentialPurpose::QrzDirectory,
        };
        let saved = credentials::put_in(
            &mut tx,
            &wrapping,
            identity,
            None,
            material(b"rotation-provider-secret"),
        )
        .await?;
        identities.push(saved);
    }
    tx.commit().await?;
    let target_file = directory.path().join("W2");
    std::fs::write(&target_file, [20u8; 32])?;
    let wrapping = WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "W2".into(),
        files: [("W1".into(), file), ("W2".into(), target_file.clone())].into(),
    })?;
    let only_target = WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "W2".into(),
        files: [("W2".into(), target_file)].into(),
    })?;
    let request = AccountRotationRequest {
        account: owner,
        from_version: 1,
        to_version: 2,
        batch_size: NonZeroU16::new(1).context("invalid batch")?,
    };
    let mut connection = PgConnection::connect(&url).await?;
    rotation::cutover(&mut connection, &wrapping, &request).await?;
    assert_eq!(
        migration::batch(
            &mut connection,
            &wrapping,
            &request,
            CiphertextFamily::Credential
        )
        .await?,
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
    assert_eq!(progress.migrated.credentials, 0);
    assert_eq!(progress.migrated.receipts, 0);
    assert!(matches!(
        progress.family,
        Some(CiphertextFamily::Credential)
    ));
    assert!(std::error::Error::source(progress).is_some());
    let diagnostic = format!("{missing_source:#}");
    assert!(diagnostic.contains(&owner.to_string()));
    assert!(diagnostic.contains("credential "));
    assert!(
        identities
            .iter()
            .any(|saved| diagnostic.contains(&saved.identity.credential.to_string()))
    );

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
    assert_eq!(interrupted.migrated.credentials, 0);
    assert_eq!(interrupted.remaining.credentials, 2);

    let report = rotation::rotate(
        PgConnection::connect(&url).await?,
        &wrapping,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(report.end, MaintenanceEnd::Complete);
    assert_eq!(report.migrated.credentials, 2);
    assert_eq!(report.remaining.credentials, 0);
    let again = rotation::rotate(
        PgConnection::connect(&url).await?,
        &wrapping,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(again.migrated.credentials, 0);
    let stale = AccountRotationRequest {
        from_version: 2,
        to_version: 4,
        ..request
    };
    assert!(
        rotation::rotate(
            PgConnection::connect(&url).await?,
            &wrapping,
            &stale,
            &CancellationToken::new()
        )
        .await
        .is_err()
    );

    let mut tx = pool.begin().await?;
    for original in identities {
        let stored = credentials::read_in(&mut tx, &wrapping, original.identity)
            .await?
            .context("missing credential")?;
        assert_eq!(
            stored.metadata.material_revision,
            original.material_revision
        );
        assert_eq!(stored.metadata.created_at, original.created_at);
        assert_eq!(stored.metadata.changed_at, original.changed_at);
        assert_eq!(
            stored
                .material
                .context("missing material")?
                .bytes
                .as_slice(),
            b"rotation-provider-secret"
        );
    }
    let active = crate::secrets::keys::load_version(&mut *tx, &wrapping, owner, 2).await?;
    assert_eq!(active.version, 2);
    tx.commit().await?;
    let events = std::fs::read_to_string(output.path())?;
    assert!(events.contains("account.key.cutover"));
    assert!(events.contains(&owner.to_string()));
    assert!(events.contains("\"to_version\":2"));
    assert!(events.contains("\"credentials\":2"));
    assert!(!events.contains("rotation-provider-secret"));
    let metric = snapshot
        .snapshot()
        .into_vec()
        .into_iter()
        .find(|(key, _, _, _)| {
            key.key().name() == "clacks_secret_values_reencrypted_total"
                && key
                    .key()
                    .labels()
                    .any(|label| label.key() == "family" && label.value() == "credential")
        })
        .context("missing committed credential count")?;
    assert_eq!(metric.3, metrics_util::debugging::DebugValue::Counter(3));
    assert_eq!(metric.0.key().labels().count(), 1);

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
    let store = OAuthStore::new(pool.clone(), wrapping.clone());
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
    let request = AccountRotationRequest {
        account: principal.account_id,
        from_version: 1,
        to_version: 2,
        batch_size: NonZeroU16::new(1).context("invalid batch")?,
    };
    let report = rotation::rotate(
        PgConnection::connect(&url).await?,
        &wrapping,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(report.migrated.receipts, 1);
    store.put_authorization_code(taken).await?;
    let mut connection = pool.acquire().await?;
    let counts =
        crate::maintenance::keys::references::count(&mut connection, principal.account_id, 2)
            .await?;
    assert_eq!(counts.receipts, 2);
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
