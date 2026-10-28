//! Durable provider registration is reusable and independent of database setup.

use crate::support;

use clacks_server::{
    config::{ProviderConfig, RegistrationMode},
    maintenance::registration,
};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn registration_reuses_saved_client_and_preserves_it_on_mismatch() -> anyhow::Result<()> {
    let provider = support::provider::start().await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("registration.json");
    let config = ProviderConfig {
        issuer: provider.issuer.clone(),
        label: "Test".into(),
        registration: RegistrationMode::Dynamic {
            registration_file: path.clone(),
            initial_access_token_file: None,
        },
    };
    let callback = url::Url::parse("https://clacks.example/auth/callback/test")?;
    registration::run(&config, &callback).await?;
    let original = std::fs::read(&path)?;
    registration::run(&config, &callback).await?;
    assert_eq!(provider.registrations.load(Ordering::SeqCst), 1);
    assert!(
        registration::run(&config, &url::Url::parse("https://other.example/callback")?)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&path)?, original);
    Ok(())
}
