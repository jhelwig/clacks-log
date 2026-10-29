//! Durable workspace binding after native credentials have been saved.

use super::{AuthorizedEnrollment, flow};
use crate::{
    instance::{self, Election},
    workspace::{AppDirectories, WorkspacePaths},
};
use anyhow::Context;
use clacks_client_store::{
    enrollment::{self, EnrollmentAttempt, EnrollmentPhase, RecoveryDisposition},
    identity::{self, EnrollmentBinding},
    store::{ClientStore, StoreSettings},
};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

/// Durable completion result retained until launch and obsolete-credential cleanup succeed.
pub(crate) struct CompletedWorkspace {
    /// Account-qualified workspace to open through ordinary application startup.
    pub destination: PathBuf,
    /// Exact saved attempt eligible for clearing after subsequent composition succeeds.
    pub attempt: EnrollmentAttempt,
    /// Committed verified binding.
    pub binding: EnrollmentBinding,
}

/// Bind a verified result without repurposing the initiating standalone account.
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
        attempt.credential_reference == authorized.credential_reference
            && flow::origin(&attempt.server)? == flow::origin(&authorized.server)?,
        "The saved attempt differs from the authorized credential"
    );
    let expected = attempt.expected.clone();
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
    let destination = match attempt.destination.clone() {
        Some(directory) => directory,
        None if expected.is_none() => AppDirectories::discover()?
            .data
            .join("accounts")
            .join(next.account.to_string())
            .join(next.actor.to_string()),
        None => anyhow::bail!("The saved recovery attempt has no workspace directory"),
    };
    anyhow::ensure!(
        !cancellation.is_cancelled(),
        "Enrollment completion cancelled; saved credentials remain available for retry"
    );
    let paths = WorkspacePaths::at(&destination)?;
    attempt.destination = Some(paths.directory.clone());
    if attempt.phase != EnrollmentPhase::DestinationBound {
        attempt.phase = EnrollmentPhase::CredentialSaved;
    }
    enrollment::save_attempt(initiating, &attempt).await?;

    let outcome = match &expected {
        None => RecoveryDisposition::New,
        Some(previous) if previous.actor == next.actor => RecoveryDisposition::SameActor,
        Some(previous) => RecoveryDisposition::FreshActor {
            former_actor: previous.actor,
        },
    };
    let current = identity::binding(initiating).await?;
    if expected.is_some() && (current == expected || current.as_ref() == Some(&next)) {
        install(initiating, expected.as_ref(), &next, outcome).await?;
    } else {
        let _owner = match instance::elect(&paths)? {
            Election::Owner(owner) => owner,
            Election::Contended(address) => {
                instance::activation::request_async(&address).await?;
                anyhow::bail!(
                    "Another Clacks process owns the destination workspace. Its window was activated; retry completion after it closes."
                );
            }
        };
        let existed = paths.database.try_exists()?;
        let opened = async {
            if existed {
                ClientStore::open(&paths.database, next.account, StoreSettings::default()).await
            } else {
                ClientStore::create(&paths.database, next.account, StoreSettings::default()).await
            }
        }
        .await;
        let destination_store = match opened {
            Ok(store) => store,
            Err(error) => {
                if !existed
                    || matches!(
                        error.downcast_ref::<clacks_client_store::error::StoreError>(),
                        Some(clacks_client_store::error::StoreError::WrongAccount { .. })
                    )
                {
                    retain_rejected_destination(initiating, &mut attempt).await?;
                }
                return Err(error.context("The destination could not be opened. Correct an uninstalled destination, or restore access and resume an existing one"));
            }
        };
        let installed = install(&destination_store, expected.as_ref(), &next, outcome).await;
        let retained: anyhow::Result<()> = async {
            if installed.is_err()
                && identity::binding(&destination_store).await?.as_ref() != Some(&next)
            {
                retain_rejected_destination(initiating, &mut attempt).await?;
            }
            Ok(())
        }
        .await;
        let closed = destination_store.close().await;
        retained?;
        installed?;
        closed?;
    }

    attempt.phase = EnrollmentPhase::DestinationBound;
    enrollment::save_attempt(initiating, &attempt).await?;
    tracing::info!(account_id=%next.account,actor_id=%next.actor,"Verified enrollment installed; awaiting synchronization");
    Ok(CompletedWorkspace {
        destination: paths.directory,
        attempt,
        binding: next,
    })
}

/// Accept an already completed exact binding or perform the guarded installation once.
async fn install(
    store: &ClientStore,
    expected: Option<&EnrollmentBinding>,
    next: &EnrollmentBinding,
    outcome: RecoveryDisposition,
) -> anyhow::Result<()> {
    if identity::binding(store).await?.as_ref() == Some(next) {
        return Ok(());
    }
    enrollment::install(store, expected, next.clone(), outcome).await
}

/// Keep a first-enrollment path editable after a known pre-binding rejection.
async fn retain_rejected_destination(
    initiating: &ClientStore,
    attempt: &mut EnrollmentAttempt,
) -> anyhow::Result<()> {
    if attempt.expected.is_none() && attempt.phase != EnrollmentPhase::DestinationBound {
        attempt.phase = EnrollmentPhase::NeedsDestination;
        enrollment::save_attempt(initiating, attempt).await?;
    }
    Ok(())
}
