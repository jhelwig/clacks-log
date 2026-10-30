//! Deployment-key rotation preserves inactive keys and actual replay containment.

use super::*;
use crate::{
    auth::oidc::VerifiedIdentity,
    enrollment::ApprovalPurpose,
    maintenance::keys::{references, rewrap},
    oauth::{config, store::OAuthStore},
    store::AccountStore,
};
use huskarl_core::dpop::DPoP;
use huskarl_crypto_native::asymmetric::signer::{GenerateAlgorithm, PrivateKey};
use std::sync::Arc;
use time::Duration;

#[tokio::test]
async fn rewrap_preserves_inactive_keys_and_rejects_missing_recovery_material() -> anyhow::Result<()>
{
    let _maintenance = super::MAINTENANCE_TEST.lock().await;
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshot = recorder.snapshotter();
    let _metrics_guard = metrics::set_default_local_recorder(&recorder);
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
                issuer: "https://rewrap-provider.example".into(),
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
    let directory = tempfile::tempdir()?;
    let file1 = directory.path().join("W1");
    let file2 = directory.path().join("W2");
    std::fs::write(&file1, [41u8; 32])?;
    std::fs::write(&file2, [42u8; 32])?;
    let key_identity = ulid::Ulid::generate();
    let from = format!("{key_identity}-W1");
    let to = format!("{key_identity}-W2");
    let source = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: from.clone(),
        files: [(from.clone(), file1.clone())].into(),
    })?);
    let target = WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: to.clone(),
        files: [(from.clone(), file1), (to.clone(), file2.clone())].into(),
    })?;
    let only_new = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: to.clone(),
        files: [(to.clone(), file2)].into(),
    })?);
    let store = OAuthStore::new(pool.clone(), source.clone());
    let issuer = config::issuer(store.clone(), &"https://clacks.example".parse()?);
    let dpop = DPoP::builder()
        .signer(PrivateKey::generate(GenerateAlgorithm::Es256, None)?)
        .build();
    let issued = crate::store::tests::enrollment::approve(
        &issuer,
        &store,
        &principal,
        &dpop,
        "Desktop",
        ApprovalPurpose::New,
    )
    .await?;
    let identity = CredentialIdentity {
        account: owner,
        credential: CredentialId::new(),
        purpose: CredentialPurpose::QrzDirectory,
    };
    let mut tx = pool.begin().await?;
    lock(&mut tx, owner).await?;
    let original = credentials::put_in(
        &mut tx,
        &source,
        identity,
        None,
        material(b"rewrap-provider-secret"),
    )
    .await?;
    tx.commit().await?;
    let request = AccountRotationRequest {
        account: owner,
        from_version: 1,
        to_version: 2,
        batch_size: NonZeroU16::new(1).context("invalid batch")?,
    };
    let mut connection = PgConnection::connect(&url).await?;
    rotation::cutover(&mut connection, &source, &request).await?;
    migration::batch(
        &mut connection,
        &source,
        &request,
        CiphertextFamily::Credential,
    )
    .await?;
    let partial = references::status(&mut connection, Some(owner)).await?;
    let references::KeyStatus::Account { versions, .. } = partial else {
        anyhow::bail!("wrong status scope")
    };
    assert_eq!(versions.len(), 2);
    assert!(
        versions
            .iter()
            .any(|row| row.version == 1 && !row.active && row.references.receipts == 1)
    );
    assert!(
        versions
            .iter()
            .any(|row| row.version == 2 && row.active && row.references.credentials == 1)
    );
    let mut tx = connection.begin().await?;
    let missing_key = credentials::read_in(&mut tx, &only_new, identity)
        .await
        .err()
        .context("missing wrapping key was accepted")?;
    assert!(format!("{missing_key:#}").contains(&from));
    tx.rollback().await?;
    connection.close().await?;

    let request = rewrap::RewrapRequest {
        from: from.clone(),
        to: to.clone(),
        batch_size: NonZeroU16::new(1).context("invalid batch")?,
    };
    let wrong_file = directory.path().join("wrong-retained-key");
    std::fs::write(&wrong_file, [99u8; 32])?;
    let wrong = WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: to.clone(),
        files: [
            (from.clone(), wrong_file),
            (to.clone(), directory.path().join("W2")),
        ]
        .into(),
    })?;
    let failure = rewrap::run(
        PgConnection::connect(&url).await?,
        &wrong,
        &request,
        &CancellationToken::new(),
    )
    .await
    .err()
    .context("wrong retained material was accepted")?;
    let progress = failure
        .downcast_ref::<crate::maintenance::keys::report::RewrapError>()
        .context("rewrap failure lost committed progress")?;
    assert_eq!(progress.migrated_account_keys, 0);
    assert_eq!(progress.account, Some(owner));
    assert_eq!(progress.key_version, Some(1));
    assert!(std::error::Error::source(progress).is_some());
    let diagnostic = format!("{failure:#}");
    assert!(diagnostic.contains(&owner.to_string()));
    assert!(diagnostic.contains("key version 1"));

    let result = rewrap::run(
        PgConnection::connect(&url).await?,
        &target,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(result.end, MaintenanceEnd::Complete);
    assert_eq!(result.migrated_account_keys, 2);
    assert_eq!(result.remaining_account_keys, 0);
    let again = rewrap::run(
        PgConnection::connect(&url).await?,
        &target,
        &request,
        &CancellationToken::new(),
    )
    .await?;
    assert_eq!(again.migrated_account_keys, 0);
    let mut tx = pool.begin().await?;
    let saved = credentials::read_in(&mut tx, &only_new, identity)
        .await?
        .context("missing material")?;
    assert_eq!(saved.metadata.material_revision, original.material_revision);
    assert_eq!(
        saved.material.context("missing material")?.bytes.as_slice(),
        b"rewrap-provider-secret"
    );
    tx.commit().await?;
    let new_store = OAuthStore::new(pool.clone(), only_new);
    let new_issuer = config::issuer(new_store.clone(), &"https://clacks.example".parse()?);
    assert!(
        new_store
            .authorized(&issued.response.access_token)
            .await?
            .is_some()
    );
    assert!(
        crate::store::tests::enrollment::exchange(
            &new_issuer,
            &dpop,
            &issued.code,
            "http://127.0.0.1:49123/oauth/callback",
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        )
        .await
        .is_err()
    );
    assert!(
        new_store
            .authorized(&issued.response.access_token)
            .await?
            .is_none()
    );
    let metric = snapshot
        .snapshot()
        .into_vec()
        .into_iter()
        .find(|(key, _, _, _)| key.key().name() == "clacks_account_keys_rewrapped_total")
        .context("missing committed rewrap count")?;
    assert_eq!(metric.3, metrics_util::debugging::DebugValue::Counter(2));
    assert_eq!(metric.0.key().labels().count(), 0);
    pool.close().await;

    Ok(())
}
