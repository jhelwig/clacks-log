//! Session-owned connection orchestration with private completion recovery.
//!
//! `clacks_desktop_enrollment_operations_total` counts admitted operations, with
//! bounded completed/cancelled/cleanup_failed/failed outcomes. Completion includes
//! cleanup; cleanup failure never implies that a committed binding was rolled back.
//! `clacks_desktop_enrollment_duration_seconds` covers the whole operation, including
//! browser wait, under the same outcome labels. Account IDs and inputs are not labels.

#[cfg(test)]
mod tests;

use super::{
    EnrollmentCommand, EnrollmentRequest, EnrollmentStatus, completion, credentials,
    failure::{self, Problem},
    flow,
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

/// Read presentation context without exposing internal attempt management.
pub(crate) async fn inspect(store: &ClientStore) -> anyhow::Result<EnrollmentStatus> {
    let binding = identity::binding(store).await?;
    let attempt = enrollment::attempt(store).await?;
    Ok(EnrollmentStatus::Idle {
        server: binding
            .as_ref()
            .map(|binding| binding.server.clone())
            .or_else(|| attempt.as_ref().map(|attempt| attempt.server().to_owned())),
        enrolled: binding.is_some(),
    })
}

/// Run one admitted operation; only the session publishes its terminal outcome.
#[tracing::instrument(skip_all, name = "desktop.enrollment.workflow")]
pub(crate) async fn execute(
    store: ClientStore,
    paths: WorkspacePaths,
    command: EnrollmentCommand,
    status: watch::Sender<EnrollmentStatus>,
    cancellation: CancellationToken,
) -> EnrollmentStatus {
    let started = std::time::Instant::now();
    status.send_replace(EnrollmentStatus::Saving);
    let result = run(&store, &paths, command, &status, cancellation.clone()).await;
    let outcome = match &result {
        Ok(()) => "completed",
        Err(_) if cancellation.is_cancelled() => "cancelled",
        Err(error) if matches!(error.downcast_ref::<Problem>(), Some(Problem::Cleanup)) => {
            "cleanup_failed"
        }
        Err(_) => "failed",
    };
    metrics::counter!("clacks_desktop_enrollment_operations_total", "outcome" => outcome)
        .increment(1);
    metrics::histogram!("clacks_desktop_enrollment_duration_seconds", "outcome" => outcome)
        .record(started.elapsed().as_secs_f64());
    match result {
        Ok(()) => EnrollmentStatus::Ready,
        Err(_) if cancellation.is_cancelled() => {
            tracing::info!("Connection cancelled; internal completion state retained");
            inspect(&store)
                .await
                .unwrap_or_else(|error| failure::present(&error))
        }
        Err(error) => {
            tracing::warn!(outcome, error.type = crate::telemetry::failure::error_class(&error),
                "Connection operation did not complete");
            let mut failed = failure::present(&error);
            if let EnrollmentStatus::Failed { enrolled, .. } = &mut failed {
                *enrolled = identity::binding(&store).await.ok().flatten().is_some();
            }
            failed
        }
    }
}

/// Resolve connection intent against actual installed/native state.
async fn run(
    store: &ClientStore,
    paths: &WorkspacePaths,
    command: EnrollmentCommand,
    status: &watch::Sender<EnrollmentStatus>,
    cancellation: CancellationToken,
) -> anyhow::Result<()> {
    let EnrollmentCommand::Connect { server, label } = command else {
        anyhow::bail!("No connection was requested");
    };
    let server = flow::origin(&server).context(Problem::Input)?.to_string();
    anyhow::ensure!(
        !label.trim().is_empty() && label.chars().count() <= 128,
        Problem::Input
    );
    let binding = identity::binding(store).await.context(Problem::Storage)?;
    if let Some(binding) = &binding {
        anyhow::ensure!(
            flow::origin(&binding.server)? == flow::origin(&server)?,
            Problem::Server
        );
    } else {
        enrollment::first::check_eligible(store).await?;
    }
    let mut saved = enrollment::attempt(store).await.context(Problem::Storage)?;
    if let Some(attempt) = &saved {
        if flow::origin(attempt.server())? != flow::origin(&server)? {
            anyhow::ensure!(
                binding.is_none()
                    && attempt.expected().is_none()
                    && attempt.phase() != EnrollmentPhase::DestinationBound,
                Problem::Server
            );
            let reference = attempt.credential_reference().to_owned();
            tokio::task::spawn_blocking(move || match credentials::load(&reference) {
                Ok(_) => credentials::remove(&reference),
                Err(error) if failure::missing(&error) => Ok(()),
                Err(error) => Err(error),
            })
            .await?
            .context(Problem::Credentials)?;
            enrollment::clear_attempt(store, attempt)
                .await
                .context(Problem::Storage)?;
            saved = None;
        } else if let Some(binding) = &binding {
            if binding.credential_reference == attempt.credential_reference() {
                // SQLite can commit before the phase update. The exact installed
                // reference and origin establish completion without a network call.
                cleanup(store, attempt).await.context(Problem::Cleanup)?;
                tracing::info!("Connection completion cleanup finished");
                return Ok(());
            }
            anyhow::ensure!(attempt.expected() == Some(binding), Problem::Storage);
        } else {
            anyhow::ensure!(
                attempt.expected().is_none()
                    && attempt.phase() != EnrollmentPhase::DestinationBound,
                Problem::Storage
            );
        }
    }
    let mut attempt = match saved {
        Some(attempt) => attempt,
        None => create_attempt(store, paths, &server, binding).await?,
    };
    anyhow::ensure!(!cancellation.is_cancelled(), "Connection cancelled");
    let reference = attempt.credential_reference().to_owned();
    let loaded = tokio::task::spawn_blocking(move || credentials::load(&reference)).await?;
    let mut authorized = None;
    match loaded {
        Ok(bundle) if bundle.refresh.is_some() => {
            let refreshed = tokio::select! {
                _ = cancellation.cancelled() => anyhow::bail!("Connection cancelled"),
                result = flow::resume(attempt.credential_reference().to_owned()) => result,
            };
            match refreshed {
                Ok(value)
                    if attempt
                        .expected()
                        .is_none_or(|expected| expected.account == value.identity.account) =>
                {
                    tracing::info!("Saved authorization verified");
                    authorized = Some(value);
                }
                Ok(_) => {
                    attempt = restart_attempt(store, paths).await?;
                }
                Err(error) if failure::refresh_rejected(&error) => {
                    attempt = restart_attempt(store, paths).await?;
                }
                Err(error) => return Err(error.context(Problem::Authorization)),
            }
        }
        Ok(_) => {}
        Err(error) if failure::missing(&error) => {
            attempt = restart_attempt(store, paths).await?;
        }
        Err(error) => return Err(error.context(Problem::Credentials)),
    }
    let authorized = match authorized {
        Some(authorized) => authorized,
        None => {
            tracing::info!("Browser authorization required for connection");
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
            .await
            .map_err(|error| {
                if error.downcast_ref::<Problem>().is_some() {
                    error
                } else {
                    error.context(Problem::Authorization)
                }
            })?
        }
    };
    status.send_replace(EnrollmentStatus::Saving);
    attempt.credential_saved();
    enrollment::save_attempt(store, &attempt)
        .await
        .context(Problem::Storage)?;
    let completed = completion::finish(store, &authorized, cancellation)
        .await
        .context(Problem::Storage)?;
    cleanup(&completed.store, &completed.attempt)
        .await
        .context(Problem::Cleanup)?;
    Ok(())
}

/// Persist a new native key and its nonsecret context before authorization starts.
async fn create_attempt(
    store: &ClientStore,
    paths: &WorkspacePaths,
    server: &str,
    expected: Option<identity::EnrollmentBinding>,
) -> anyhow::Result<EnrollmentAttempt> {
    let native_server = server.to_owned();
    let reference = tokio::task::spawn_blocking(move || credentials::create(&native_server))
        .await?
        .context(Problem::Credentials)?;
    let attempt = EnrollmentAttempt::new(
        reference.clone(),
        server.to_owned(),
        Some(paths.directory.clone()),
        expected,
    )?;
    if let Err(error) = enrollment::save_attempt(store, &attempt).await {
        let _ = tokio::task::spawn_blocking(move || credentials::remove(&reference)).await;
        return Err(error.context(Problem::Storage));
    }
    Ok(attempt)
}

/// Remove only superseded material after the new connection is installed.
async fn cleanup(store: &ClientStore, attempt: &EnrollmentAttempt) -> anyhow::Result<()> {
    if let Some(previous) = attempt.expected()
        && previous.credential_reference != attempt.credential_reference()
    {
        let reference = previous.credential_reference.clone();
        tokio::task::spawn_blocking(move || credentials::remove(&reference)).await??;
    }
    enrollment::clear_attempt(store, attempt).await
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
