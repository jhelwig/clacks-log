//! Durable enrollment of the workspace owned by the current application instance.

use super::{AuthorizedEnrollment, flow};
use anyhow::Context;
use clacks_client_store::{
    enrollment::{self, EnrollmentAttempt, RecoveryDisposition},
    identity::{self, EnrollmentBinding},
    store::ClientStore,
};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

/// Committed enrollment retained until obsolete-credential cleanup succeeds.
pub(crate) struct CompletedWorkspace {
    /// Existing workspace directory; enrollment does not launch another process.
    pub destination: PathBuf,
    /// Exact saved attempt eligible for clearing after cleanup.
    pub attempt: EnrollmentAttempt,
    /// Committed verified binding.
    pub binding: EnrollmentBinding,
    /// Same database qualified by the newly installed account identity.
    pub store: ClientStore,
}

/// Enroll this workspace, rejecting populated standalone data according to D56.
#[tracing::instrument(skip_all, name = "desktop.enrollment.complete")]
pub(crate) async fn finish(
    initiating: &ClientStore,
    authorized: &AuthorizedEnrollment,
    cancellation: CancellationToken,
) -> anyhow::Result<CompletedWorkspace> {
    let mut attempt = enrollment::attempt(initiating)
        .await?
        .context("The saved enrollment attempt is missing")?;
    anyhow::ensure!(
        attempt.credential_reference() == authorized.credential_reference
            && flow::origin(attempt.server())? == flow::origin(&authorized.server)?,
        "The saved attempt differs from the authorized credential"
    );
    let expected = attempt.expected().cloned();
    if let Some(previous) = &expected {
        anyhow::ensure!(
            previous.account == authorized.identity.account
                && flow::origin(&previous.server)? == flow::origin(&authorized.server)?,
            "The authorized account or server differs from this workspace"
        );
    }
    let next = EnrollmentBinding {
        account: authorized.identity.account,
        actor: authorized.identity.actor,
        server: expected
            .as_ref()
            .map(|previous| previous.server.clone())
            .unwrap_or_else(|| authorized.server.clone()),
        credential_reference: authorized.credential_reference.clone(),
    };
    let destination = attempt
        .destination()
        .cloned()
        .context("The saved attempt has no current workspace directory")?;
    anyhow::ensure!(
        !cancellation.is_cancelled(),
        "Enrollment completion cancelled; saved credentials remain available for retry"
    );
    attempt.prepare_completion(destination.clone())?;
    enrollment::save_attempt(initiating, &attempt).await?;
    let current = identity::binding(initiating).await?;
    let store = if current.as_ref() == Some(&next) {
        initiating.clone()
    } else if let Some(previous) = &expected {
        let outcome = if previous.actor == next.actor {
            RecoveryDisposition::SameActor
        } else {
            RecoveryDisposition::FreshActor {
                former_actor: previous.actor,
            }
        };
        enrollment::install(initiating, Some(previous), next.clone(), outcome).await?;
        initiating.clone()
    } else {
        enrollment::first::install(initiating, &next).await?
    };
    attempt.destination_bound()?;
    enrollment::save_attempt(&store, &attempt).await?;
    tracing::info!(account_id=%next.account,actor_id=%next.actor,"Verified enrollment installed; awaiting synchronization");
    Ok(CompletedWorkspace {
        destination,
        attempt,
        binding: next,
        store,
    })
}
