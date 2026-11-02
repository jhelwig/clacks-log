//! Ordered installation of complete native values with explicit local retry.

#[cfg(test)]
mod tests;

use super::credentials;
use clacks_client_store::{
    directory::installation, identity::EnrollmentBinding, store::ClientStore,
};
use clacks_sync::protocol::directory::ProvisionedConfiguration;
use std::sync::Arc;
use tokio::sync::{Notify, watch};
use tokio_util::sync::CancellationToken;

/// Committed configuration transitions, excluding unchanged or failed native writes.
pub(crate) const CONFIGURATION_INSTALLS_METRIC: &str =
    "clacks_directory_configuration_installs_total";

/// Complete delivery owned by one authenticated connection lifetime.
#[derive(Clone)]
pub(crate) struct Delivery {
    /// Authoritative configuration with redacted secret diagnostics.
    pub configuration: ProvisionedConfiguration,
    /// End of the delivering connection.
    pub connection_stop: CancellationToken,
}

/// Directory-only diagnostic; logging and synchronization remain independent.
#[derive(Clone, Debug, Default)]
pub(crate) enum ProvisioningStatus {
    /// No unresolved native access failure.
    #[default]
    Idle,
    /// Safe diagnostic accompanied by the local retry action.
    Failed(String),
}

/// Existing worker publication and one coalesced explicit retry signal.
#[derive(Clone)]
pub(crate) struct Outputs {
    /// Complete last usable installation; secret diagnostics remain redacted.
    pub installed: watch::Sender<Option<ProvisionedConfiguration>>,
    /// Current local storage diagnosis.
    pub status: watch::Sender<ProvisioningStatus>,
    /// Explicit retry wakes the same worker without a server round trip.
    pub retry: Arc<Notify>,
}

/// Check delivery ownership before beginning the non-interruptible native operation.
fn current(
    delivery: &Delivery,
    latest: &watch::Receiver<Option<Delivery>>,
    stop: &CancellationToken,
) -> bool {
    !stop.is_cancelled()
        && !delivery.connection_stop.is_cancelled()
        && latest
            .borrow()
            .as_ref()
            .is_some_and(|latest| latest.configuration == delivery.configuration)
}

/// Completed native operation and whether its stored value actually changed.
struct NativeInstallation {
    /// Complete value read or written by the operation.
    configuration: ProvisionedConfiguration,
    /// False when the stored value already matched the delivery.
    changed: bool,
}

/// Replace one complete native value, using no cross-store installation transaction.
#[tracing::instrument(skip_all, name="desktop.directory.install", fields(account_id=%binding.account))]
pub(crate) async fn install(
    reference: &str,
    binding: &EnrollmentBinding,
    delivery: &Delivery,
    latest: &watch::Receiver<Option<Delivery>>,
    stop: &CancellationToken,
) -> anyhow::Result<Option<ProvisionedConfiguration>> {
    if !current(delivery, latest, stop) {
        return Ok(None);
    }
    delivery.configuration.validate()?;
    anyhow::ensure!(
        delivery.configuration.account == binding.account,
        "Directory provisioning crossed account ownership"
    );
    let reference = reference.to_owned();
    let binding = binding.clone();
    let delivery = delivery.clone();
    let latest = latest.clone();
    let stop = stop.clone();
    let installed = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let previous = credentials::load(&binding, &reference).ok().flatten();
        if !current(&delivery, &latest, &stop) {
            return Ok(None);
        }
        if previous.as_ref() == Some(&delivery.configuration) {
            return Ok(Some(NativeInstallation {
                configuration: delivery.configuration,
                changed: false,
            }));
        }
        credentials::save(&binding, &reference, &delivery.configuration)?;
        Ok(Some(NativeInstallation {
            configuration: delivery.configuration,
            changed: true,
        }))
    })
    .await??;
    let Some(NativeInstallation {
        configuration,
        changed,
    }) = installed
    else {
        return Ok(None);
    };
    if changed {
        let outcome = if configuration.configuration.selected.is_none() {
            "disabled"
        } else if configuration.material.is_none() {
            "unconfigured"
        } else {
            "selected"
        };
        metrics::counter!(CONFIGURATION_INSTALLS_METRIC, "outcome"=>outcome).increment(1);
        tracing::info!(event="directory.configuration.installed", account_id=%configuration.account, revision=configuration.configuration.revision, outcome);
    }
    Ok(Some(configuration))
}

/// Load or retry local access and apply the retained current delivery on the existing worker.
pub(crate) async fn run(
    store: ClientStore,
    binding: EnrollmentBinding,
    mut incoming: watch::Receiver<Option<Delivery>>,
    outputs: Outputs,
    stop: CancellationToken,
) {
    loop {
        let result = async {
            let reference = installation::reference(&store).await?;
            let owner = binding.clone();
            let key = reference.clone();
            let read = tokio::task::spawn_blocking(move || credentials::load(&owner, &key)).await?;
            if let Ok(Some(configuration)) = &read {
                outputs.installed.send_replace(Some(configuration.clone()));
            }
            let delivery = incoming.borrow_and_update().clone();
            if let Some(delivery) = delivery
                && let Some(configuration) =
                    install(&reference, &binding, &delivery, &incoming, &stop).await?
            {
                outputs.installed.send_replace(Some(configuration));
                return Ok(());
            }
            read.map(|_| ())
        }
        .await;
        match result {
            Ok(()) => {
                outputs.status.send_replace(ProvisioningStatus::Idle);
            }
            Err(error) => {
                tracing::warn!(event="directory.provisioning.failed", error.type=crate::telemetry::failure::error_class(&error));
                outputs.status.send_replace(ProvisioningStatus::Failed("Directory storage could not be accessed. The last usable configuration is retained; retry directory storage when access is available.".into()));
            }
        }
        tokio::select! { _ = stop.cancelled() => break, _ = outputs.retry.notified() => {}, changed = incoming.changed() => if changed.is_err() { break } }
    }
}
