//! Session startup, persisted identity and ordered resource shutdown.

use crate::{
    instance::{self, Election},
    session::{self, SessionCommand, SessionResources, SessionStatus},
    telemetry::TelemetryGuard,
    workspace::WorkspacePaths,
};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::{mpsc, watch};

/// Run real ownership, startup and cleanup without requiring a display or collector daemon.
pub(super) fn complete(
    runtime: &tokio::runtime::Runtime,
    paths: &WorkspacePaths,
    telemetry: TelemetryGuard,
) -> anyhow::Result<()> {
    let Election::Owner(owner) = instance::elect(paths)? else {
        anyhow::bail!("owner expected")
    };
    let (notices, activation) = watch::channel(());

    let listener = owner.listen(runtime.handle(), notices)?;
    let (commands, input) = mpsc::channel(8);
    let (updates, mut status) = watch::channel(SessionStatus::Opening);
    let (_settings_requests, requests) = mpsc::channel(8);
    let (settings_updates, _settings_view) = watch::channel(None);
    let resources = SessionResources {
        directory: crate::directory::provisioning::Outputs {
            installed: tokio::sync::watch::channel(None).0,
            status: tokio::sync::watch::channel(Default::default()).0,
            retry: Default::default(),
        },
        enrollment_updates: tokio::sync::watch::channel(Default::default()).0,
        settings_requests: requests,
        settings_updates,
        paths: paths.clone(),
        listener,
        telemetry,
        activation,
        context: Arc::new(OnceLock::new()),
    };
    runtime.block_on(async {
        let task = tokio::spawn(session::run(resources, input, updates));
        let identity = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let SessionStatus::Ready { identity, .. } = *status.borrow_and_update() {
                    return anyhow::Ok(identity);
                }
                status.changed().await?;
            }
        })
        .await??;
        commands.send(SessionCommand::BeginShutdown).await?;
        task.await??;

        assert!(matches!(*status.borrow(), SessionStatus::Closed));
        let store = clacks_client_store::store::ClientStore::open_discovered(
            &paths.database,
            Default::default(),
        )
        .await?;

        assert_eq!(store.account_id(), identity.account);
        store.close().await?;
        anyhow::Ok(())
    })?;
    drop(owner);

    assert!(matches!(instance::elect(paths)?, Election::Owner(_)));
    Ok(())
}
