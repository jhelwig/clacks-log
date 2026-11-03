//! Session-owned enrollment orchestration and explicit durable completion retries.

#[cfg(test)]
mod tests;

use super::{
    EnrollmentCommand, EnrollmentRequest, EnrollmentStatus, completion, credentials, flow,
};
use crate::workspace::WorkspacePaths;
use anyhow::Context;
use clacks_client_store::{
    enrollment::{self, EnrollmentAttempt, EnrollmentPhase},
    identity,
    store::ClientStore,
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Durable attempt and label for an explicit browser approval when needed.
struct RequestedAttempt {
    /// Persisted nonsecret completion boundary.
    attempt: EnrollmentAttempt,
    /// User-visible label for a newly requested authorization.
    label: String,
}

/// Read local connection context, checking native state before offering credential removal.
pub(crate) async fn inspect(store: &ClientStore) -> anyhow::Result<EnrollmentStatus> {
    let binding = identity::binding(store).await?;
    let attempt = enrollment::attempt(store).await?;
    let discard_available = match &attempt {
        Some(attempt) => discardable(attempt).await.unwrap_or(false),
        None => false,
    };

    Ok(EnrollmentStatus::Idle {
        server: attempt
            .as_ref()
            .map(|attempt| attempt.server().to_owned())
            .or_else(|| binding.as_ref().map(|binding| binding.server.clone())),
        enrolled: binding.is_some(),

        resume_available: attempt.is_some(),
        discard_available,
    })
}

/// Run one admitted workflow, returning its outcome for publication after the session joins it.
#[tracing::instrument(skip_all, name = "desktop.enrollment.workflow")]
pub(crate) async fn execute(
    store: ClientStore,
    paths: WorkspacePaths,
    command: EnrollmentCommand,
    status: watch::Sender<EnrollmentStatus>,
    cancellation: CancellationToken,
) -> EnrollmentStatus {
    status.send_replace(EnrollmentStatus::Saving);
    let result = run(&store, &paths, command, &status, cancellation.clone()).await;

    match result {
        Ok(Some(completed)) => EnrollmentStatus::Ready {
            destination: completed.destination,
        },
        Ok(None) => match inspect(&store).await {
            Ok(status) => status,
            Err(error) => EnrollmentStatus::Failed {
                message: error.to_string(),
                resume_available: false,
                discard_available: false,
            },
        },
        Err(error) => {
            let attempt = enrollment::attempt(&store).await.ok().flatten();
            let discard_available = match &attempt {
                Some(attempt) => discardable(attempt).await.unwrap_or(false),
                None => false,
            };
            if cancellation.is_cancelled() {
                tracing::info!(
                    "Desktop authorization cancelled; durable completion state retained"
                );
            } else {
                tracing::warn!(
                    error.type = crate::telemetry::failure::error_class(&error),
                    "Desktop enrollment did not complete; retry information is available in the connection dialog"
                );
            }
            EnrollmentStatus::Failed {
                message: format!("{error:#}"),
                resume_available: attempt.is_some(),

                discard_available,
            }
        }
    }
}

/// Permit abandoning only a key without saved refresh credentials, or an already absent entry.
///
/// Native persistence can precede SQLite's phase update. An unreadable native
/// entry is not evidence that its credentials are safe to remove.
async fn discardable(attempt: &EnrollmentAttempt) -> anyhow::Result<bool> {
    if attempt.phase() != EnrollmentPhase::KeySaved {
        return Ok(false);
    }

    let reference = attempt.credential_reference().to_owned();
    tokio::task::spawn_blocking(move || match credentials::load(&reference) {
        Ok(bundle) => Ok(bundle.refresh.is_none()),
        Err(error)
            if matches!(
                error.downcast_ref::<keyring_core::Error>(),
                Some(keyring_core::Error::NoEntry)
            ) =>
        {
            Ok(true)
        }
        Err(error) => Err(error),
    })
    .await?
}

/// Carry out one normal attempt, preserving completion state until binding and credential cleanup succeed.
async fn run(
    store: &ClientStore,
    paths: &WorkspacePaths,
    command: EnrollmentCommand,
    status: &watch::Sender<EnrollmentStatus>,
    cancellation: CancellationToken,
) -> anyhow::Result<Option<completion::CompletedWorkspace>> {
    if matches!(command, EnrollmentCommand::Discard) {
        let attempt = enrollment::attempt(store)
            .await?
            .context("No saved enrollment attempt exists")?;
        anyhow::ensure!(
            discardable(&attempt).await?,
            "This attempt has saved credentials or may already be installed. Resume completion instead of discarding its key."
        );
        anyhow::ensure!(
            identity::binding(store).await?.is_none_or(
                |binding| binding.credential_reference != attempt.credential_reference()
            ),
            "An installed credential cannot be discarded as an unfinished attempt"
        );
        let reference = attempt.credential_reference().to_owned();
        tokio::task::spawn_blocking(move || credentials::remove(&reference)).await??;
        enrollment::clear_attempt(store, &attempt).await?;
        return Ok(None);
    }
    let RequestedAttempt { mut attempt, label } = match command {
        EnrollmentCommand::Begin { server, label } => {
            anyhow::ensure!(
                enrollment::attempt(store).await?.is_none(),
                "A saved enrollment attempt already exists. Resume it or discard its uninstalled key first."
            );
            let server = flow::origin(&server)?.to_string();
            anyhow::ensure!(
                !label.trim().is_empty() && label.chars().count() <= 128,
                "Enter a client label of 1–128 characters"
            );
            let expected = identity::binding(store).await?;
            if let Some(binding) = &expected {
                anyhow::ensure!(
                    flow::origin(&binding.server)? == flow::origin(&server)?,
                    "Reauthorization must use this workspace's existing server"
                );
            }
            let native_server = server.clone();
            let reference =
                tokio::task::spawn_blocking(move || credentials::create(&native_server)).await??;
            let attempt =
                EnrollmentAttempt::new(reference, server, Some(paths.directory.clone()), expected)?;
            if let Err(error) = enrollment::save_attempt(store, &attempt).await {
                let reference = attempt.credential_reference().to_owned();
                let removed =
                    tokio::task::spawn_blocking(move || credentials::remove(&reference)).await;
                if !matches!(removed, Ok(Ok(()))) {
                    tracing::warn!(
                        "Could not remove the unused proof key after saving its attempt failed"
                    );
                }
                return Err(error.context("Could not save the new enrollment attempt"));
            }
            RequestedAttempt { attempt, label }
        }
        EnrollmentCommand::AuthorizeAgain { label } => RequestedAttempt {
            attempt: restart_attempt(store, paths).await?,
            label,
        },
        EnrollmentCommand::Resume { label } => RequestedAttempt {
            attempt: enrollment::attempt(store)
                .await?
                .context("No saved enrollment attempt exists; choose Connect")?,
            label,
        },
        EnrollmentCommand::Cancel | EnrollmentCommand::Discard => {
            anyhow::bail!("No authorization workflow was selected")
        }
    };
    let reference = attempt.credential_reference().to_owned();
    let bundle = tokio::task::spawn_blocking(move || credentials::load(&reference)).await??;
    let authorized = if bundle.refresh.is_some() {
        tokio::select! { _ = cancellation.cancelled() => anyhow::bail!("Completion cancelled; the saved attempt is available for retry"), result = flow::resume(attempt.credential_reference().to_owned()) => result? }
    } else {
        flow::authorize(
            EnrollmentRequest {
                server: attempt.server().to_owned(),
                label,
                credential_reference: attempt.credential_reference().to_owned(),
                expected: attempt.expected().cloned(),
            },
            status.clone(),
            cancellation.clone(),
        )
        .await?
    };
    status.send_replace(EnrollmentStatus::Saving);
    attempt.credential_saved();
    enrollment::save_attempt(store, &attempt).await?;
    let completed = completion::finish(store, &authorized, cancellation).await?;
    if let Some(previous) = &completed.attempt.expected()
        && previous.credential_reference != completed.binding.credential_reference
    {
        let reference = previous.credential_reference.clone();
        tokio::task::spawn_blocking(move || credentials::remove(&reference)).await?
            .context("Enrollment is installed, but superseded credential removal failed. Resume completion to retry cleanup")?;
    }
    enrollment::clear_attempt(&completed.store, &completed.attempt).await?;
    Ok(Some(completed))
}

/// Start fresh browser approval while retaining any binding already installed by this attempt.
pub(super) async fn restart_attempt(
    store: &ClientStore,
    paths: &WorkspacePaths,
) -> anyhow::Result<EnrollmentAttempt> {
    let previous = enrollment::attempt(store)
        .await?
        .context("No saved enrollment attempt exists")?;
    let mut expected = previous.expected().cloned();
    if let Some(binding) = identity::binding(store).await? {
        anyhow::ensure!(
            (Some(&binding) == previous.expected()
                || binding.credential_reference == previous.credential_reference())
                && flow::origin(&binding.server)? == flow::origin(previous.server())?,
            "The workspace enrollment changed; its current binding was preserved"
        );
        expected = Some(binding);
    }
    let server = previous.server().to_owned();
    let reference = tokio::task::spawn_blocking(move || credentials::create(&server)).await??;
    let next = EnrollmentAttempt::new(
        reference.clone(),
        previous.server().to_owned(),
        Some(paths.directory.clone()),
        expected,
    )?;
    if let Err(error) = enrollment::save_attempt(store, &next).await {
        let _ = tokio::task::spawn_blocking(move || credentials::remove(&reference)).await;
        return Err(error.context("Could not save the restarted authorization attempt"));
    }
    if next
        .expected()
        .as_ref()
        .is_none_or(|binding| binding.credential_reference != previous.credential_reference())
    {
        tokio::task::spawn_blocking(move || credentials::remove(previous.credential_reference()))
            .await??;
    }
    Ok(next)
}
