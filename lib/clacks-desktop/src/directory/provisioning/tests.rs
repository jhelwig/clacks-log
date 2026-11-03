//! Native replacement, rollback and explicit local recovery use the actual selected backend.

#![cfg(any(target_os = "macos", windows))]

/// Remove only this fixture's stable native entry, including on failure.
struct NativeEntry(String);
impl Drop for NativeEntry {
    fn drop(&mut self) {
        let _ = crate::directory::credentials::remove(&self.0);
    }
}

#[tokio::test]
async fn directory_installation_does_not_use_revision_as_freshness() -> anyhow::Result<()> {
    use crate::directory::{
        credentials,
        provisioning::{self, Delivery, Outputs, ProvisioningStatus},
    };
    use clacks_client_store::{
        directory::installation, identity::EnrollmentBinding, store::ClientStore,
    };
    use clacks_model::{
        directory::values::{DirectoryConfiguration, DirectoryProvider},
        ids::{AccountId, ClientActorId, DirectoryMaterialId},
    };
    use clacks_sync::protocol::directory::{
        DirectoryLogin, DirectoryMaterial, ProvisionedConfiguration,
    };
    use tokio::sync::watch;
    use tokio_util::sync::CancellationToken;
    let temporary = tempfile::tempdir()?;
    let store = ClientStore::create(
        &temporary.path().join("workspace.sqlite"),
        AccountId::new(),
        Default::default(),
    )
    .await?;
    let binding = EnrollmentBinding {
        account: store.account_id(),
        actor: ClientActorId::new(),
        server: "https://directory.example".into(),
        credential_reference: "unchanged-enrollment-reference".into(),
    };
    clacks_client_store::identity::bind(&store, None, binding.clone()).await?;
    let reference = installation::reference(&store).await?;
    let _entry = NativeEntry(reference.clone());
    let first = Delivery {
        configuration: ProvisionedConfiguration {
            account: binding.account,
            configuration: DirectoryConfiguration {
                revision: 5,
                selected: Some(DirectoryProvider::Qrz),
                configured: [DirectoryProvider::Qrz].into(),
            },
            material: Some(DirectoryMaterial {
                id: DirectoryMaterialId::new(),
                revision: 1,
                login: DirectoryLogin {
                    username: "fixture".into(),
                    password: "original-provisioned-secret".into(),
                },
            }),
        },
        connection_stop: CancellationToken::new(),
    };

    // An unreadable owned native value is an input fixture, not a mocked OS failure.
    crate::credentials::entry("com.w7hex.clacks-log.directory", &reference)?
        .set_secret(b"unreadable saved directory value")?;
    let (_deliveries, incoming) = watch::channel(None);
    let (installed, mut view) = watch::channel(None);
    let (status, mut diagnosis) = watch::channel(Default::default());
    let outputs = Outputs {
        installed,
        status,
        retry: Default::default(),
    };
    let retry = outputs.retry.clone();
    let local_stop = CancellationToken::new();
    let worker = tokio::spawn(provisioning::run(
        store.clone(),
        binding.clone(),
        incoming,
        outputs,
        local_stop.clone(),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if matches!(
                *diagnosis.borrow_and_update(),
                ProvisioningStatus::Failed(_)
            ) {
                break;
            }
            diagnosis.changed().await?;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    credentials::save(&binding, &reference, &first.configuration)?;
    retry.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if view.borrow_and_update().as_ref() == Some(&first.configuration) {
                break;
            }
            view.changed().await?;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    local_stop.cancel();
    worker.await?;

    let (updates, latest) = watch::channel(Some(first.clone()));
    let stop = CancellationToken::new();
    let installed = provisioning::install(&reference, &binding, &first, &latest, &stop)
        .await?
        .unwrap();
    assert_eq!(
        credentials::load(&binding, &reference)?,
        Some(installed.clone())
    );
    let mut restored = first.clone();
    restored.configuration.configuration.revision = 1;
    restored
        .configuration
        .material
        .as_mut()
        .unwrap()
        .login
        .password = "restored-server-secret".into();
    updates.send_replace(Some(restored.clone()));
    assert!(
        provisioning::install(&reference, &binding, &first, &latest, &stop)
            .await?
            .is_none(),
        "an obsolete queued delivery must not overwrite the newer configuration"
    );
    assert_eq!(credentials::load(&binding, &reference)?, Some(installed));
    let replaced = provisioning::install(&reference, &binding, &restored, &latest, &stop)
        .await?
        .unwrap();
    assert_eq!(replaced.configuration.revision, 1);
    assert_eq!(
        credentials::load(&binding, &reference)?,
        Some(restored.configuration.clone())
    );
    assert_eq!(
        provisioning::install(&reference, &binding, &restored, &latest, &stop).await?,
        Some(replaced)
    );
    let mut same_revision = restored.clone();
    same_revision
        .configuration
        .material
        .as_mut()
        .unwrap()
        .login
        .password = "same-revision-new-secret".into();
    updates.send_replace(Some(same_revision.clone()));
    let installed = provisioning::install(&reference, &binding, &same_revision, &latest, &stop)
        .await?
        .unwrap();
    assert_eq!(
        credentials::load(&binding, &reference)?,
        Some(same_revision.configuration.clone())
    );
    restored.connection_stop.cancel();
    assert!(
        provisioning::install(&reference, &binding, &restored, &latest, &stop)
            .await?
            .is_none()
    );
    assert_eq!(credentials::load(&binding, &reference)?, Some(installed));
    let removed = Delivery {
        configuration: ProvisionedConfiguration {
            account: binding.account,
            configuration: DirectoryConfiguration {
                revision: 2,
                selected: Some(DirectoryProvider::Qrz),
                configured: Default::default(),
            },
            material: None,
        },
        connection_stop: CancellationToken::new(),
    };
    updates.send_replace(Some(removed.clone()));
    let unconfigured = provisioning::install(&reference, &binding, &removed, &latest, &stop)
        .await?
        .unwrap();
    assert!(unconfigured.material.is_none());
    assert_eq!(credentials::load(&binding, &reference)?, Some(unconfigured));
    assert_eq!(
        clacks_client_store::identity::binding(&store).await?,
        Some(binding)
    );
    store.close().await?;
    Ok(())
}
