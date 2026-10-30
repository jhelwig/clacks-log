//! Durable completion of verified enrollment without silently readmitting old work.

mod attempt;
pub(crate) mod holds;

use crate::{
    identity::{self, EnrollmentBinding},
    local_state,
    store::ClientStore,
};
use clacks_model::ids::ClientActorId;
use serde::{Deserialize, Serialize};

/// Existing local-state key holding one non-secret completion attempt.
const ATTEMPT: &str = "enrollment.attempt";

/// Last durable boundary reached by a desktop enrollment workflow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnrollmentPhase {
    /// Signing key is durable before opening the browser.
    KeySaved,
    /// Complete native credential is durable before binding SQLite.
    CredentialSaved,
    /// The destination was rejected before this credential was installed; its path may be corrected.
    NeedsDestination,
    /// Exact verified binding committed in the destination workspace.
    DestinationBound,
}

/// Non-secret state required to resume an interrupted enrollment completion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "attempt::AttemptFields", into = "attempt::AttemptFields")]
pub struct EnrollmentAttempt {
    /// Opaque native-store entry reference, never usable key/token material.
    credential_reference: String,
    /// Selected server origin, retained independently of callback parameters.
    server: String,
    /// Coherent first-enrollment or recovery progress.
    mode: attempt::EnrollmentMode,
}

/// Verified relationship between the former and newly authorized writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryDisposition {
    /// First enrollment of an empty workspace.
    New,
    /// Credentials changed while the writer identity remains authorized.
    SameActor,
    /// A closed/unknown writer is replaced without rewriting its pending work.
    FreshActor {
        /// Identity that originally owned the pending proposals.
        former_actor: ClientActorId,
    },
}

/// Read the non-secret workflow boundary saved by this workspace.
pub async fn attempt(store: &ClientStore) -> anyhow::Result<Option<EnrollmentAttempt>> {
    let mut transaction = store.pool.begin().await?;
    let value = local_state::get_in(&mut transaction, ATTEMPT).await?;
    transaction.commit().await?;
    value
        .map(|value| serde_json::from_str(&value).map_err(Into::into))
        .transpose()
}

/// Persist a completion boundary after its native-store or destination operation succeeds.
pub async fn save_attempt(store: &ClientStore, attempt: &EnrollmentAttempt) -> anyhow::Result<()> {
    let mut transaction = store.write().await?;
    local_state::set_in(&mut transaction, ATTEMPT, &serde_json::to_string(attempt)?).await?;
    transaction.commit().await?;
    Ok(())
}

/// Clear only the exact completed attempt, leaving a newer workflow untouched.
pub async fn clear_attempt(store: &ClientStore, attempt: &EnrollmentAttempt) -> anyhow::Result<()> {
    let encoded = serde_json::to_string(attempt)?;
    let mut transaction = store.write().await?;
    sqlx::query_file!("sql/enrollment/clear_attempt.sql", ATTEMPT, encoded)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

/// Commit verified binding and any old-writer admission holds under the exact prior binding.
#[tracing::instrument(skip_all,name="client_store.enrollment.install",fields(account_id=%store.account,disposition=?outcome))]
pub async fn install(
    store: &ClientStore,
    expected: Option<&EnrollmentBinding>,
    next: EnrollmentBinding,
    outcome: RecoveryDisposition,
) -> anyhow::Result<()> {
    let valid = match (expected, outcome) {
        (None, RecoveryDisposition::New) => true,
        (Some(previous), RecoveryDisposition::SameActor) => {
            previous.actor == next.actor && previous.server == next.server
        }
        (Some(previous), RecoveryDisposition::FreshActor { former_actor }) => {
            previous.actor == former_actor
                && previous.actor != next.actor
                && previous.server == next.server
        }
        _ => false,
    };
    anyhow::ensure!(valid, "enrollment outcome does not match the prior binding");
    let mut transaction = store.write().await?;
    identity::bind_in(store, &mut transaction, expected, &next).await?;
    if let RecoveryDisposition::FreshActor { former_actor } = outcome {
        sqlx::query_file!(
            "sql/enrollment/hold_pending.sql",
            former_actor.to_string(),
            store.account.to_string()
        )
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    tracing::info!(event="workspace.enrollment.bound",account_id=%store.account,actor_id=%next.actor,disposition=?outcome);
    Ok(())
}
