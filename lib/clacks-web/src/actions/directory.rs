//! Account-authorized directory actions with nonsecret browser responses.

use clacks_model::directory::values::{DirectoryConfiguration, DirectoryProvider};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Editable account setup, excluding saved passwords and their lengths.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectorySetup {
    /// Current selection and saved-provider availability.
    pub configuration: DirectoryConfiguration,
    /// Saved usernames keyed by their provider.
    pub usernames: std::collections::BTreeMap<DirectoryProvider, String>,
}

/// Public outcome of one complete form submission, never containing saved login bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DirectorySaveOutcome {
    /// The submitted semantic change committed.
    Committed(DirectoryConfiguration),
    /// Current values already matched the submission.
    Unchanged(DirectoryConfiguration),
    /// Another submission changed the displayed basis; review current configuration.
    Stale(DirectoryConfiguration),
}

/// Read directory selection and credential availability for the signed-in account.
#[server]
pub async fn load_directory() -> Result<DirectorySetup, ServerFnError> {
    let state = super::state()?;
    let principal = super::principal(&state).await?;
    let setup = clacks_server::directory::setup(&state, &principal)
        .await
        .map_err(clacks_server::auth::error::AuthError::from_store)?;
    Ok(DirectorySetup {
        configuration: setup.configuration,
        usernames: setup.usernames,
    })
}

/// Save an explicit configuration/material intent; secrets never appear in its response.
#[server]
pub async fn save_directory(
    expected_revision: i64,
    selected: String,
    operation: String,
    provider: String,
    username: String,
    password: String,
) -> Result<DirectorySaveOutcome, ServerFnError> {
    use clacks_server::auth::error::AuthError;
    use clacks_server::directory::{CredentialChange, DirectoryUpdate, DirectoryUpdateOutcome};
    let state = super::state()?;
    let principal = super::principal(&state).await?;
    let selected = if selected.is_empty() {
        None
    } else {
        Some(
            selected
                .parse()
                .map_err(|_| ServerFnError::new("Choose QRZ, HamQTH or disabled."))?,
        )
    };
    let credential = match operation.as_str() {
        "keep" => CredentialChange::Keep,
        "remove" => CredentialChange::Remove {
            provider: provider
                .parse()
                .map_err(|_| ServerFnError::new("Choose the credential provider."))?,
        },
        "save" => CredentialChange::Save {
            provider: provider
                .parse()
                .map_err(|_| ServerFnError::new("Choose the credential provider."))?,
            username,
            password: (!password.is_empty()).then_some(password),
        },
        _ => {
            return Err(ServerFnError::new("Unknown directory submission."));
        }
    };
    let result = if operation == "keep" {
        clacks_server::directory::update(
            &state,
            &principal,
            DirectoryUpdate {
                expected_revision,
                selected,
                credential,
            },
        )
        .await
    } else {
        clacks_server::directory::update_credentials(
            &state,
            &principal,
            expected_revision,
            credential,
        )
        .await
    }
    .map_err(|error| {
        match error.downcast_ref::<clacks_server::directory::CredentialInputError>() {
            Some(error) => ServerFnError::new(error.to_string()),
            None => AuthError::from_store(error).into(),
        }
    })?;
    Ok(match result {
        DirectoryUpdateOutcome::Committed(configuration) => {
            DirectorySaveOutcome::Committed(configuration)
        }
        DirectoryUpdateOutcome::Unchanged(configuration) => {
            DirectorySaveOutcome::Unchanged(configuration)
        }
        DirectoryUpdateOutcome::Stale(configuration) => DirectorySaveOutcome::Stale(configuration),
    })
}
