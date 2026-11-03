//! Short account-locked transactions for actual directory configuration/material.

use super::{
    AccountStore,
    rows::directory::{ConfigurationRow, MaterialRow},
};
use crate::{
    auth::principal::Principal,
    directory::{CredentialChange, CredentialInputError, DirectoryUpdate, DirectoryUpdateOutcome},
    secrets::{
        directory::{self as protected, DirectoryContext},
        envelope::DirectoryEnvelope,
        keys::{self, WrappingKeys},
    },
};
use clacks_model::{
    directory::values::{DirectoryConfiguration, DirectoryProvider},
    ids::{AccountId, DirectoryMaterialId},
};
use clacks_sync::protocol::directory::DirectoryMaterial;
use sqlx::PgConnection;

/// Read the account's editable usernames in one consistent snapshot.
pub(crate) async fn setup(
    store: &AccountStore,
    wrapping: &WrappingKeys,
    principal: &Principal,
) -> anyhow::Result<crate::directory::DirectorySetup> {
    let mut tx = store
        .pool
        .begin_with("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    let configuration = configuration_in(&mut tx, principal.account_id).await?;
    let mut usernames = std::collections::BTreeMap::new();
    for provider in [DirectoryProvider::Qrz, DirectoryProvider::HamQth] {
        if let Some(material) = material(&mut tx, wrapping, principal.account_id, provider).await? {
            usernames.insert(provider, material.login.username.clone());
        }
    }
    tx.commit().await?;
    Ok(crate::directory::DirectorySetup {
        configuration,
        usernames,
    })
}

/// Read a consistent public configuration without provisioning a key or secret.
pub(crate) async fn load(
    store: &AccountStore,
    principal: &Principal,
) -> anyhow::Result<DirectoryConfiguration> {
    let mut tx = store
        .pool
        .begin_with("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    let configuration = configuration_in(&mut tx, principal.account_id).await?;
    tx.commit().await?;
    Ok(configuration)
}

/// Load public selection/material availability inside the caller's transaction.
pub(crate) async fn configuration_in(
    connection: &mut PgConnection,
    account: AccountId,
) -> anyhow::Result<DirectoryConfiguration> {
    let row = sqlx::query_file_as!(
        ConfigurationRow,
        "sql/directory/configuration.sql",
        account.to_string()
    )
    .fetch_optional(&mut *connection)
    .await?;
    let providers = sqlx::query_file!("sql/directory/providers.sql", account.to_string())
        .fetch_all(&mut *connection)
        .await?;
    let configured = providers
        .into_iter()
        .map(|r| r.provider.parse())
        .collect::<anyhow::Result<_>>()?;
    let configuration = match row {
        Some(row) => DirectoryConfiguration {
            revision: row.revision,
            selected: row.selected_provider.map(|p| p.parse()).transpose()?,
            configured,
        },
        None => DirectoryConfiguration {
            configured,
            ..Default::default()
        },
    };
    anyhow::ensure!(
        configuration.revision >= 0,
        "negative directory configuration revision"
    );
    Ok(configuration)
}

/// Authenticate one complete directory row with its independently expected owner/purpose.
pub(crate) async fn material(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    account: AccountId,
    provider: DirectoryProvider,
) -> anyhow::Result<Option<DirectoryMaterial>> {
    let row = sqlx::query_file_as!(
        MaterialRow,
        "sql/directory/material.sql",
        account.to_string(),
        provider.as_str()
    )
    .fetch_optional(&mut *connection)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    anyhow::ensure!(
        row.account_id == account.to_string() && row.provider == provider.as_str(),
        "directory row crossed owner/purpose"
    );
    let context = DirectoryContext {
        account,
        provider,
        material_id: row.material_id.parse()?,
        material_revision: row.material_revision,
    };
    let envelope = DirectoryEnvelope::parse(&row.encrypted_envelope)?;
    let key =
        keys::load_version(&mut *connection, wrapping, account, envelope.key_version()).await?;
    Ok(Some(protected::open(&key, &context, &envelope)?))
}

/// Compare a public revision and commit the entire submitted intent under the account lock.
pub(crate) async fn update(
    store: &AccountStore,
    wrapping: &WrappingKeys,
    principal: &Principal,
    change: DirectoryUpdate,
) -> anyhow::Result<DirectoryUpdateOutcome> {
    update_in(store, wrapping, principal, change, false).await
}

/// Apply a credential-only edit without overwriting selection from another form.
pub(crate) async fn update_preserving_selection(
    store: &AccountStore,
    wrapping: &WrappingKeys,
    principal: &Principal,
    change: DirectoryUpdate,
) -> anyhow::Result<DirectoryUpdateOutcome> {
    update_in(store, wrapping, principal, change, true).await
}

/// Resolve the submitted intent under the existing account lock.
async fn update_in(
    store: &AccountStore,
    wrapping: &WrappingKeys,
    principal: &Principal,
    mut change: DirectoryUpdate,
    preserve_selection: bool,
) -> anyhow::Result<DirectoryUpdateOutcome> {
    let account = principal.account_id;
    anyhow::ensure!(
        change.expected_revision >= 0,
        "invalid directory configuration revision"
    );
    let mut tx = store.pool.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", account.to_string())
        .fetch_one(&mut *tx)
        .await?;
    let authority = sqlx::query_file!(
        "sql/directory/principal.sql",
        account.to_string(),
        principal.identity_id.to_string()
    )
    .fetch_one(&mut *tx)
    .await?;
    anyhow::ensure!(
        authority.present,
        "account login identity is no longer linked"
    );
    let current = configuration_in(&mut tx, account).await?;
    if current.revision != change.expected_revision {
        return Ok(DirectoryUpdateOutcome::Stale(current));
    }
    if preserve_selection {
        change.selected = current.selected;
    }
    if let CredentialChange::Save {
        provider,
        ref username,
        ref password,
    } = change.credential
    {
        let password = match password {
            Some(password) => password.clone(),
            None => material(&mut tx, wrapping, account, provider)
                .await?
                .ok_or(CredentialInputError::MissingPassword)?
                .login
                .password
                .clone(),
        };
        change.credential = CredentialChange::Replace {
            provider,
            login: clacks_sync::protocol::directory::DirectoryLogin {
                username: username.clone(),
                password,
            },
        };
    }
    let mut changed = current.selected != change.selected;
    match change.credential {
        CredentialChange::Keep => {}
        CredentialChange::Save { .. } => unreachable!("save resolved before applying material"),
        CredentialChange::Remove { provider } => {
            let result = sqlx::query_file!(
                "sql/directory/remove_material.sql",
                account.to_string(),
                provider.as_str()
            )
            .execute(&mut *tx)
            .await?;
            changed |= result.rows_affected() != 0;
        }
        CredentialChange::Replace { provider, login } => {
            if login.username.trim().is_empty() {
                return Err(CredentialInputError::MissingUsername.into());
            }
            if login.password.is_empty() {
                return Err(CredentialInputError::MissingPassword.into());
            }
            let previous = material(&mut tx, wrapping, account, provider).await?;
            if previous.as_ref().is_none_or(|old| old.login != login) {
                let material = match previous {
                    Some(mut old) => {
                        old.revision = old.revision.checked_add(1).ok_or_else(|| {
                            anyhow::anyhow!("directory material revision exhausted")
                        })?;
                        old.login = login;
                        old
                    }
                    None => DirectoryMaterial {
                        id: DirectoryMaterialId::new(),
                        revision: 1,
                        login,
                    },
                };
                let context = DirectoryContext {
                    account,
                    provider,
                    material_id: material.id,
                    material_revision: material.revision,
                };
                let key = keys::active_in(&mut tx, wrapping, account).await?;
                let envelope = protected::seal(&key, &context, &material)?.encode();
                sqlx::query_file!(
                    "sql/directory/upsert_material.sql",
                    account.to_string(),
                    provider.as_str(),
                    material.id.to_string(),
                    material.revision,
                    envelope
                )
                .execute(&mut *tx)
                .await?;
                changed = true;
            }
        }
    }
    if !changed {
        return Ok(DirectoryUpdateOutcome::Unchanged(current));
    }
    let revision = current
        .revision
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("directory configuration revision exhausted"))?;
    let selected = change.selected.map(DirectoryProvider::as_str);
    sqlx::query_file!(
        "sql/directory/upsert_configuration.sql",
        account.to_string(),
        revision,
        selected
    )
    .execute(&mut *tx)
    .await?;
    let committed = configuration_in(&mut tx, account).await?;
    tx.commit().await?;
    let _ = store.changes.send(account);
    tracing::info!(event="directory.configuration.changed", account_id=%account, revision, "Directory configuration committed");
    Ok(DirectoryUpdateOutcome::Committed(committed))
}
