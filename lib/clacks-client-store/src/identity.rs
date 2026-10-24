//! Enrollment metadata supplied by verified account authentication.

use crate::rows::workspace::EnrollmentRow;
use crate::{error::StoreError, store::ClientStore};
use clacks_model::ids::{AccountId, ClientActorId};
use sqlx::SqliteConnection;

/// Verified server enrollment and a reference to protected local credential storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnrollmentBinding {
    /// Native account selected during authentication.
    pub account: AccountId,
    /// Server endpoint for this enrollment.
    pub server: String,
    /// Current authorized writer identity.
    pub actor: ClientActorId,
    /// Opaque protected-store reference, never the credential itself.
    pub credential_reference: String,
}

/// Account identity and optional current enrollment from one database read.
pub(crate) struct WorkspaceIdentity {
    /// Owning account regardless of enrollment.
    pub(crate) account: AccountId,
    /// Current verified enrollment, if any.
    pub(crate) binding: Option<EnrollmentBinding>,
}

/// Decode complete enrollment fields without accepting partially populated bindings.
pub(crate) async fn load(
    connection: &mut SqliteConnection,
) -> anyhow::Result<Option<WorkspaceIdentity>> {
    let Some(row) = sqlx::query_file_as!(EnrollmentRow, "sql/opening/identity.sql")
        .fetch_optional(connection)
        .await?
    else {
        return Ok(None);
    };
    let account = row.account.parse()?;
    let binding = match row {
        EnrollmentRow {
            server: None,
            actor: None,
            credential_reference: None,
            ..
        } => None,
        EnrollmentRow {
            server: Some(server),
            actor: Some(actor),
            credential_reference: Some(credential_reference),
            ..
        } if !server.is_empty() && !credential_reference.is_empty() => Some(EnrollmentBinding {
            account,
            server,
            actor: actor.parse()?,
            credential_reference,
        }),
        _ => {
            return Err(StoreError::InvalidDatabase {
                detail: "partial enrollment binding".into(),
            }
            .into());
        }
    };
    Ok(Some(WorkspaceIdentity { account, binding }))
}

/// Install enrollment under an exact prior-binding check.
///
/// First enrollment requires an empty workspace. Explicit same-account reauthorization
/// changes the binding without rewriting prior authors or pending work.
#[tracing::instrument(skip_all, name = "client_store.enrollment.bind")]
pub async fn bind(
    store: &ClientStore,
    expected: Option<&EnrollmentBinding>,
    binding: EnrollmentBinding,
) -> anyhow::Result<()> {
    if binding.account != store.account {
        return Err(StoreError::WrongAccount {
            expected: store.account,
            actual: binding.account,
        }
        .into());
    }
    anyhow::ensure!(
        !binding.server.trim().is_empty() && !binding.credential_reference.trim().is_empty(),
        StoreError::InvalidDatabase {
            detail: "empty enrollment endpoint or credential reference".into()
        }
    );
    let mut transaction = store.write().await?;
    let current = load(&mut transaction)
        .await?
        .ok_or_else(|| StoreError::InvalidDatabase {
            detail: "account row is missing".into(),
        })?;
    anyhow::ensure!(
        current.binding.as_ref() == expected,
        StoreError::StaleBinding
    );
    if current.binding.is_none() {
        let row = sqlx::query_file!("sql/workspace/populated.sql")
            .fetch_one(&mut *transaction)
            .await?;
        anyhow::ensure!(row.populated == 0, StoreError::PopulatedWorkspace);
    }
    let actor = binding.actor.to_string();
    let account = binding.account.to_string();
    sqlx::query_file!(
        "sql/workspace/bind.sql",
        binding.server,
        actor,
        binding.credential_reference,
        account
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    tracing::info!(event = "workspace.enrollment.bound", account_id = %store.account);
    Ok(())
}

/// Non-secret native workspace identity for local application presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceStatus {
    /// Account qualifying all records in the workspace.
    pub account: AccountId,
    /// Whether the workspace has a complete installed enrollment binding.
    pub enrolled: bool,
}

/// Read non-secret workspace status without exposing protected credential references.
///
/// Use during application startup or after changing enrollment. This does not
/// contact a server or establish whether any connection is currently available.
pub async fn status(store: &ClientStore) -> anyhow::Result<WorkspaceStatus> {
    let mut transaction = store.pool.begin().await?;
    let identity = load(&mut transaction)
        .await?
        .ok_or_else(|| StoreError::InvalidDatabase {
            detail: "account row is missing".into(),
        })?;
    anyhow::ensure!(
        identity.account == store.account,
        StoreError::WrongAccount {
            expected: store.account,
            actual: identity.account
        }
    );
    let result = WorkspaceStatus {
        account: identity.account,
        enrolled: identity.binding.is_some(),
    };
    transaction.commit().await?;
    Ok(result)
}
