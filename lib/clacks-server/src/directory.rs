//! Browser-authorized setup for client-direct account directory lookup.

use crate::{auth::principal::Principal, runtime::ServerState};
use clacks_model::directory::values::{DirectoryConfiguration, DirectoryProvider};
use clacks_sync::protocol::directory::DirectoryLogin;

/// Safe credential validation feedback, without submitted secret bytes.
#[derive(Debug, thiserror::Error)]
pub enum CredentialInputError {
    /// A login cannot be stored without its username.
    #[error("Enter a username or callsign.")]
    MissingUsername,
    /// A new login requires a password.
    #[error("Enter a password to save credentials.")]
    MissingPassword,
}

/// Explicit material action; blank form fields do not implicitly remove secrets.
pub enum CredentialChange {
    /// Preserve all existing material.
    Keep,
    /// Save the username and, when supplied, a new password.
    Save {
        /// Provider whose login is saved.
        provider: DirectoryProvider,
        /// Editable saved username.
        username: String,
        /// New password; absence preserves the existing password.
        password: Option<String>,
    },
    /// Replace one provider's complete login.
    Replace {
        /// Provider whose material is changed.
        provider: DirectoryProvider,
        /// Exact login bytes, excluded from diagnostics.
        login: DirectoryLogin,
    },
    /// Remove one provider's saved material.
    Remove {
        /// Provider whose material is removed.
        provider: DirectoryProvider,
    },
}

/// Editable directory setup without saved passwords.
pub struct DirectorySetup {
    /// Selection and credential availability from the same snapshot.
    pub configuration: DirectoryConfiguration,
    /// Saved editable usernames.
    pub usernames: std::collections::BTreeMap<DirectoryProvider, String>,
}

/// Load editable account setup without returning saved passwords.
#[tracing::instrument(skip_all, fields(account_id=%principal.account_id))]
pub async fn setup(state: &ServerState, principal: &Principal) -> anyhow::Result<DirectorySetup> {
    crate::store::directory::setup(&state.store, &state.wrapping_keys, principal).await
}

/// Save one provider's credentials while preserving the current lookup selection.
#[tracing::instrument(skip_all, fields(account_id=%principal.account_id))]
pub async fn update_credentials(
    state: &ServerState,
    principal: &Principal,
    expected_revision: i64,
    credential: CredentialChange,
) -> anyhow::Result<DirectoryUpdateOutcome> {
    crate::store::directory::update_preserving_selection(
        &state.store,
        &state.wrapping_keys,
        principal,
        DirectoryUpdate {
            expected_revision,
            selected: None,
            credential,
        },
    )
    .await
}

/// One reviewed account form submission, applied atomically under its expected revision.
pub struct DirectoryUpdate {
    /// Revision displayed by this account's setup form.
    pub expected_revision: i64,
    /// Selection after this complete update.
    pub selected: Option<DirectoryProvider>,
    /// Explicit credential action, independent of selection.
    pub credential: CredentialChange,
}

/// Nonsecret committed/current outcome; stale writes never overwrite newer setup.
#[derive(Debug)]
pub enum DirectoryUpdateOutcome {
    /// A semantic configuration/material change committed.
    Committed(DirectoryConfiguration),
    /// The intended values were already installed.
    Unchanged(DirectoryConfiguration),
    /// The displayed revision is no longer current.
    Stale(DirectoryConfiguration),
}

/// Read only the authenticated account's public directory setup.
#[tracing::instrument(skip_all, name="account.directory.load", level="debug", fields(account_id=%principal.account_id))]
pub async fn load(
    state: &ServerState,
    principal: &Principal,
) -> anyhow::Result<DirectoryConfiguration> {
    crate::store::directory::load(&state.store, principal).await
}

/// Apply an explicit account-authorized update without performing provider network calls.
#[tracing::instrument(skip_all, name="account.directory.update", fields(account_id=%principal.account_id))]
pub async fn update(
    state: &ServerState,
    principal: &Principal,
    change: DirectoryUpdate,
) -> anyhow::Result<DirectoryUpdateOutcome> {
    crate::store::directory::update(&state.store, &state.wrapping_keys, principal, change).await
}

/// Read the selected directory material within one current-client authorized snapshot.
#[tracing::instrument(skip_all, name="account.directory.provision", level="debug", fields(account_id=%client.account, actor_id=%client.actor))]
pub(crate) async fn for_client(
    state: &ServerState,
    client: &crate::enrollment::AuthorizedClient,
) -> anyhow::Result<clacks_sync::protocol::directory::ProvisionedConfiguration> {
    let mut tx = state
        .store
        .pool
        .begin_with("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    crate::oauth::resource::check_current(&mut tx, client).await?;
    let configuration = crate::store::directory::configuration_in(&mut tx, client.account).await?;
    let material = match configuration.selected {
        Some(provider) => {
            crate::store::directory::material(
                &mut tx,
                &state.wrapping_keys,
                client.account,
                provider,
            )
            .await?
        }
        None => None,
    };
    let configuration = clacks_sync::protocol::directory::ProvisionedConfiguration {
        account: client.account,
        configuration,
        material,
    };
    configuration.validate()?;
    tx.commit().await?;
    Ok(configuration)
}
