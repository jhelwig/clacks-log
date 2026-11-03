//! Connection retries exercise the real issuer and native credential store.

use super::NativeEntry;
use crate::{
    enrollment::{self, EnrollmentCommand, EnrollmentStatus, credentials},
    workspace::{self, WorkspacePaths},
};
use clacks_client_store::{
    enrollment::{self as saved, EnrollmentAttempt},
    identity,
    store::ClientStore,
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Copy actual issued material to an independent test entry without changing its key binding.
fn copy_credential(reference: &str, origin: &str) -> anyhow::Result<NativeEntry> {
    let bundle = credentials::load(reference)?;
    let copied = credentials::create(origin)?;
    crate::credentials::entry("com.w7hex.clacks-log.authorization", &copied)?
        .set_secret(&serde_json::to_vec(&bundle)?)?;
    Ok(NativeEntry(copied))
}

/// Drive Connect until browser authorization is actually required, then cancel locally.
async fn browser_then_cancel(
    store: &ClientStore,
    paths: &WorkspacePaths,
    origin: &str,
) -> anyhow::Result<String> {
    let (updates, mut progress) = watch::channel(EnrollmentStatus::default());
    let cancellation = CancellationToken::new();
    let work = tokio::spawn(enrollment::worker::execute(
        store.clone(),
        paths.clone(),
        EnrollmentCommand::Connect {
            server: origin.into(),
            label: "Recovery test".into(),
        },
        updates,
        cancellation.clone(),
    ));
    let authorization = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            progress.changed().await?;
            if let EnrollmentStatus::Authorizing { authorization_url } = progress.borrow().clone() {
                return anyhow::Ok(authorization_url);
            }
        }
    })
    .await;
    cancellation.cancel();
    let outcome = work.await?;
    let authorization = authorization??;
    assert!(matches!(outcome, EnrollmentStatus::Idle { .. }));
    Ok(authorization)
}

/// Verify correction, missing credentials, and wrong-account recovery using real saved material.
pub(super) async fn verify(
    origin: &str,
    reference: &str,
    account: clacks_model::ids::AccountId,
) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = WorkspacePaths::at(directory.path())?;
    let store = workspace::open(&paths).await?.store;
    let copied = copy_credential(reference, origin)?;
    let attempt = EnrollmentAttempt::new(
        copied.0.clone(),
        origin.into(),
        Some(paths.directory.clone()),
        None,
    )?;
    saved::save_attempt(&store, &attempt).await?;
    // A changed server supersedes uninstalled refresh material. The new endpoint
    // is an ordinary closed loopback port, so no approval or synthetic server is needed.
    let unused = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let other = format!("http://{}", unused.local_addr()?);
    drop(unused);
    let (updates, _) = watch::channel(EnrollmentStatus::default());
    let outcome = enrollment::worker::execute(
        store.clone(),
        paths.clone(),
        EnrollmentCommand::Connect {
            server: other.clone(),
            label: "Corrected".into(),
        },
        updates,
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(outcome, EnrollmentStatus::Failed { .. }));
    assert!(matches!(
        credentials::load(&copied.0)
            .err()
            .unwrap()
            .downcast_ref::<keyring_core::Error>(),
        Some(keyring_core::Error::NoEntry)
    ));
    let corrected = saved::attempt(&store).await?.unwrap();
    let corrected_entry = NativeEntry(corrected.credential_reference().into());
    assert_eq!(corrected.server().trim_end_matches('/'), other);
    assert!(identity::binding(&store).await?.is_none());
    // Retry preserves this new key on transport failure rather than cycling credentials.
    let (updates, mut progress) = watch::channel(EnrollmentStatus::default());
    let outcome = enrollment::worker::execute(
        store.clone(),
        paths.clone(),
        EnrollmentCommand::Connect {
            server: other,
            label: "Corrected".into(),
        },
        updates,
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(outcome, EnrollmentStatus::Failed { .. }));
    assert!(matches!(
        *progress.borrow_and_update(),
        EnrollmentStatus::Saving
    ));
    assert_eq!(
        saved::attempt(&store)
            .await?
            .unwrap()
            .credential_reference(),
        corrected_entry.0
    );
    saved::clear_attempt(&store, &corrected).await?;
    // Missing material is detected internally and opens a fresh browser flow.
    let absent = EnrollmentAttempt::new(
        ulid::Ulid::generate().to_string(),
        origin.into(),
        Some(paths.directory.clone()),
        None,
    )?;
    saved::save_attempt(&store, &absent).await?;
    browser_then_cancel(&store, &paths, origin).await?;
    let replacement = saved::attempt(&store).await?.unwrap();
    let _replacement = NativeEntry(replacement.credential_reference().into());
    assert_ne!(
        replacement.credential_reference(),
        absent.credential_reference()
    );
    saved::clear_attempt(&store, &replacement).await?;
    store.close().await?;

    // A valid credential for another account cannot trap Connect in refresh retries.
    let directory = tempfile::tempdir()?;
    let paths = WorkspacePaths::at(directory.path())?;
    let expected_account = clacks_model::ids::AccountId::new();
    assert_ne!(expected_account, account);
    let store = ClientStore::create(&paths.database, expected_account, Default::default()).await?;
    let installed = NativeEntry(credentials::create(origin)?);
    let binding = identity::EnrollmentBinding {
        account: expected_account,
        actor: clacks_model::ids::ClientActorId::new(),
        server: origin.into(),
        credential_reference: installed.0.clone(),
    };
    identity::bind(&store, None, binding.clone()).await?;
    let copied = copy_credential(reference, origin)?;
    let pending = EnrollmentAttempt::new(
        copied.0.clone(),
        origin.into(),
        Some(paths.directory.clone()),
        Some(binding.clone()),
    )?;
    saved::save_attempt(&store, &pending).await?;
    let authorization = browser_then_cancel(&store, &paths, origin).await?;
    let url = url::Url::parse(&authorization)?;
    assert!(
        url.query_pairs()
            .any(|(key, value)| key == "clacks_account" && value == expected_account.to_string())
    );
    assert_eq!(identity::binding(&store).await?, Some(binding));
    assert!(credentials::load(&installed.0).is_ok());
    let next = saved::attempt(&store).await?.unwrap();
    let _next_entry = NativeEntry(next.credential_reference().into());
    store.close().await?;
    Ok(())
}

/// A revoked real grant triggers fresh browser authorization without a separate UI command.
pub(super) async fn rejected(origin: &str, reference: &str) -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = WorkspacePaths::at(directory.path())?;
    let store = workspace::open(&paths).await?.store;
    let copied = copy_credential(reference, origin)?;
    let attempt = EnrollmentAttempt::new(
        copied.0.clone(),
        origin.into(),
        Some(paths.directory.clone()),
        None,
    )?;
    saved::save_attempt(&store, &attempt).await?;
    browser_then_cancel(&store, &paths, origin).await?;
    let next = saved::attempt(&store).await?.unwrap();
    let _entry = NativeEntry(next.credential_reference().into());
    assert_ne!(next.credential_reference(), copied.0);
    store.close().await?;
    Ok(())
}
