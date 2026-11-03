//! Coherent, versioned completion state for first enrollment and recovery.

use super::{EnrollmentAttempt, EnrollmentPhase};
use crate::identity::EnrollmentBinding;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// First-enrollment progress with a required path after binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum NewProgress {
    /// Native signing key is durable; the current workspace is retained.
    KeySaved(Option<PathBuf>),
    /// Native credentials are durable; destination installation can resume.
    CredentialSaved(Option<PathBuf>),
    /// Exact destination binding is committed.
    DestinationBound(PathBuf),
}

/// Recovery progress, independent of its mandatory existing destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum RecoveryPhase {
    /// Native signing key is durable before browser authorization.
    KeySaved,
    /// Native credentials are durable before binding installation.
    CredentialSaved,
    /// Exact replacement binding is committed.
    DestinationBound,
}

/// Completion alternatives with their required recovery context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum EnrollmentMode {
    /// First enrollment in the initiating workspace.
    New(NewProgress),
    /// Recovery always retains its existing destination and prior binding.
    Recovery {
        /// Existing workspace directory.
        destination: PathBuf,
        /// Exact binding compared before replacement.
        expected: EnrollmentBinding,
        /// Last durable recovery boundary.
        phase: RecoveryPhase,
    },
}

/// Local-state input restored through checked native construction.
#[derive(Serialize, Deserialize)]
pub(super) struct AttemptFields {
    /// Opaque native-store entry reference.
    credential_reference: String,
    /// Selected server origin.
    server: String,
    /// Complete first-enrollment or recovery progress.
    mode: EnrollmentMode,
}

impl TryFrom<AttemptFields> for EnrollmentAttempt {
    type Error = anyhow::Error;

    fn try_from(fields: AttemptFields) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !fields.credential_reference.is_empty() && !fields.server.is_empty(),
            "enrollment attempt is missing its native reference or server"
        );
        Ok(Self {
            credential_reference: fields.credential_reference,
            server: fields.server,
            mode: fields.mode,
        })
    }
}

impl From<EnrollmentAttempt> for AttemptFields {
    fn from(value: EnrollmentAttempt) -> Self {
        Self {
            credential_reference: value.credential_reference,
            server: value.server,
            mode: value.mode,
        }
    }
}

impl EnrollmentAttempt {
    /// Begin a durable attempt after saving its native signing key.
    ///
    /// Recovery requires the existing workspace directory and prior binding.
    /// First enrollment retains the initiating workspace directory.
    pub fn new(
        credential_reference: String,
        server: String,
        destination: Option<PathBuf>,
        expected: Option<EnrollmentBinding>,
    ) -> anyhow::Result<Self> {
        let mode = match expected {
            Some(expected) => EnrollmentMode::Recovery {
                destination: destination
                    .context("recovery attempt requires its workspace directory")?,
                expected,
                phase: RecoveryPhase::KeySaved,
            },
            None => EnrollmentMode::New(NewProgress::KeySaved(destination)),
        };
        AttemptFields {
            credential_reference,
            server,
            mode,
        }
        .try_into()
    }

    /// Return the opaque native-store reference used to resume this attempt.
    pub fn credential_reference(&self) -> &str {
        &self.credential_reference
    }

    /// Return the server selected before browser authorization.
    pub fn server(&self) -> &str {
        &self.server
    }

    /// Borrow the retained current workspace directory.
    pub fn destination(&self) -> Option<&PathBuf> {
        match &self.mode {
            EnrollmentMode::New(
                NewProgress::KeySaved(path) | NewProgress::CredentialSaved(path),
            ) => path.as_ref(),
            EnrollmentMode::New(NewProgress::DestinationBound(path)) => Some(path),
            EnrollmentMode::Recovery { destination, .. } => Some(destination),
        }
    }

    /// Borrow the prior binding required for an existing-workspace recovery.
    pub fn expected(&self) -> Option<&EnrollmentBinding> {
        match &self.mode {
            EnrollmentMode::New(_) => None,
            EnrollmentMode::Recovery { expected, .. } => Some(expected),
        }
    }

    /// Return the last persisted completion boundary.
    pub fn phase(&self) -> EnrollmentPhase {
        match &self.mode {
            EnrollmentMode::New(NewProgress::KeySaved(_))
            | EnrollmentMode::Recovery {
                phase: RecoveryPhase::KeySaved,
                ..
            } => EnrollmentPhase::KeySaved,
            EnrollmentMode::New(NewProgress::CredentialSaved(_))
            | EnrollmentMode::Recovery {
                phase: RecoveryPhase::CredentialSaved,
                ..
            } => EnrollmentPhase::CredentialSaved,
            EnrollmentMode::New(NewProgress::DestinationBound(_))
            | EnrollmentMode::Recovery {
                phase: RecoveryPhase::DestinationBound,
                ..
            } => EnrollmentPhase::DestinationBound,
        }
    }

    /// Record completed native credential persistence without regressing a bound attempt.
    pub fn credential_saved(&mut self) {
        let destination = self.destination().cloned();
        match &mut self.mode {
            EnrollmentMode::New(
                progress @ (NewProgress::KeySaved(_) | NewProgress::CredentialSaved(_)),
            ) => {
                *progress = NewProgress::CredentialSaved(destination);
            }
            EnrollmentMode::Recovery { phase, .. } if *phase != RecoveryPhase::DestinationBound => {
                *phase = RecoveryPhase::CredentialSaved
            }
            _ => {}
        }
    }

    /// Retain the chosen completion path before attempting its binding transaction.
    pub fn prepare_completion(&mut self, destination: PathBuf) -> anyhow::Result<()> {
        match &mut self.mode {
            EnrollmentMode::New(NewProgress::DestinationBound(path))
            | EnrollmentMode::Recovery {
                destination: path, ..
            } => anyhow::ensure!(
                *path == destination,
                "an existing enrollment destination cannot be relocated"
            ),
            EnrollmentMode::New(progress) => {
                *progress = NewProgress::CredentialSaved(Some(destination))
            }
        }
        self.credential_saved();
        Ok(())
    }

    /// Record a successfully committed destination binding.
    pub fn destination_bound(&mut self) -> anyhow::Result<()> {
        let destination = self
            .destination()
            .cloned()
            .context("bound enrollment requires a destination")?;
        match &mut self.mode {
            EnrollmentMode::New(progress) => *progress = NewProgress::DestinationBound(destination),
            EnrollmentMode::Recovery { phase, .. } => *phase = RecoveryPhase::DestinationBound,
        }
        Ok(())
    }
}
