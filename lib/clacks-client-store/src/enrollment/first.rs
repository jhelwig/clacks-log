//! First enrollment replaces only unused standalone initialization in place.

use crate::{
    error::StoreError,
    identity::{self, EnrollmentBinding},
    metadata::client,
    store::ClientStore,
};
use clacks_sync::record::{RecordKey, RecordState, RecordValue};

/// Adopt the verified account in the same database, rejecting independently populated data.
///
/// Only unchanged, unpublished bundled definitions may be replaced. Local operating
/// facts and preferences survive. Callers must replace old account-qualified handles
/// with the returned handle before admitting further work.
#[tracing::instrument(skip_all, name="client_store.enrollment.first", fields(account_id=%next.account))]
pub async fn install(store: &ClientStore, next: &EnrollmentBinding) -> anyhow::Result<ClientStore> {
    anyhow::ensure!(
        !next.server.trim().is_empty() && !next.credential_reference.trim().is_empty(),
        "Enrollment requires a server and protected credential reference"
    );
    let mut transaction = store.write().await?;
    eligible_in(store, &mut transaction).await?;
    sqlx::query_file!("sql/enrollment/defer_keys.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/clear_client.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/clear_pending.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/clear_original.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/clear_observation.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/clear_payload.sql")
        .execute(&mut *transaction)
        .await?;
    let account = next.account.to_string();
    sqlx::query_file!("sql/enrollment/adopt_identity.sql", account)
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/adopt_station.sql", account)
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/adopt_location.sql", account)
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/adopt_reference.sql", account)
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/adopt_equipment.sql", account)
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/adopt_operating_value.sql", account)
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/enrollment/adopt_operating_state.sql", account)
        .execute(&mut *transaction)
        .await?;
    let actor = next.actor.to_string();
    sqlx::query_file!(
        "sql/enrollment/adopt_account.sql",
        account,
        next.server,
        actor,
        next.credential_reference
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    tracing::info!(event="workspace.enrollment.bound", account_id=%next.account, actor_id=%next.actor);
    Ok(ClientStore {
        account: next.account,
        pool: store.pool.clone(),
    })
}

/// Check whether this standalone log can connect before requesting authorization.
/// Installation repeats this check in its write transaction to reject later edits.
pub async fn check_eligible(store: &ClientStore) -> anyhow::Result<()> {
    let mut transaction = store.pool.begin().await?;
    eligible_in(store, &mut transaction).await?;
    transaction.commit().await?;
    Ok(())
}

/// Reject bindings or independently populated data within a consistent snapshot.
async fn eligible_in(
    store: &ClientStore,
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> anyhow::Result<()> {
    let current = identity::load(transaction)
        .await?
        .ok_or_else(|| anyhow::anyhow!("workspace identity is missing"))?;
    anyhow::ensure!(current.binding.is_none(), StoreError::StaleBinding);
    let definitions = clacks_model::defaults::account_definitions()?;
    let mut count = 0i64;
    for value in definitions
        .output_profiles
        .into_iter()
        .map(|value| RecordValue::OutputProfile(Box::new(value)))
        .chain(
            definitions
                .groups
                .into_iter()
                .map(|value| RecordValue::Group(Box::new(value))),
        )
    {
        let key = RecordKey {
            account: store.account,
            id: value.id(),
        };
        if let Some(record) = client::load(transaction, key).await? {
            anyhow::ensure!(
                matches!(
                    record.latest(),
                    clacks_sync::record::Observation::Unavailable { .. }
                ) && record
                    .pending()
                    .is_some_and(|pending| pending.prepared.is_none() && pending.witness.is_none())
                    && record.working_state() == Some(&RecordState::Live(value)),
                StoreError::PopulatedWorkspace
            );
            count += 1;
        }
    }
    let rows = sqlx::query_file!("sql/enrollment/first_population.sql")
        .fetch_one(&mut **transaction)
        .await?;
    anyhow::ensure!(
        rows.records == count && rows.checkpoints == 0,
        StoreError::PopulatedWorkspace
    );
    Ok(())
}
