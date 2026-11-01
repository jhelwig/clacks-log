//! Atomic installation of equivalent current contexts, reusing native payloads.

use crate::store::{
    AccountStore,
    metadata::{self, account, lineage},
    payload,
};
use clacks_model::ids::{AccountId, BaselineId};
use clacks_sync::{
    compaction::installation::plan_installation,
    record::{Position, RecordKey, VersionStamp},
    transfer::RecordRequirement,
};
use sqlx::PgConnection;
use std::time::Instant;

/// Result of the account-serialized installation operation.
pub(super) enum Outcome {
    /// Equivalent current representations were committed.
    Installed {
        /// Number of equivalent current contexts actually installed.
        converted: u64,
    },
    /// No eligible component requires a new baseline.
    Unchanged,
    /// Another account operation currently owns the write lock.
    Contended,
}

/// Current envelope ownership and its complete native context.
struct Head {
    /// Native header used by the projection calculation.
    requirement: RecordRequirement,
    /// Existing payload reused by the replacement envelope.
    payload_id: i64,
    /// Existing envelope referenced by current external bindings.
    envelope_id: i64,
}

/// Install under the existing account lock, committing no native value changes.
#[tracing::instrument(skip_all, name="server.sync.baseline.install", level="debug", fields(account_id=%owner))]
pub(super) async fn install(store: &AccountStore, owner: AccountId) -> anyhow::Result<Outcome> {
    let mut transaction = store.pool.begin().await?;
    let waiting = Instant::now();
    let lock = sqlx::query_file!("sql/retention/lock.sql", owner.to_string())
        .fetch_optional(&mut *transaction)
        .await;
    let outcome = match &lock {
        Ok(Some(_)) => "acquired",
        Ok(None) => "contended",
        Err(_) => "failed",
    };
    metrics::histogram!("clacks_sync_baseline_lock_wait_seconds", "outcome" => outcome)
        .record(waiting.elapsed().as_secs_f64());
    if lock?.is_none() {
        transaction.rollback().await?;
        return Ok(Outcome::Contended);
    }

    let holding = Instant::now();
    let result = install_locked(&mut transaction, owner).await;
    let result = match result {
        Ok(value) => transaction
            .commit()
            .await
            .map(|()| value)
            .map_err(Into::into),
        Err(error) => {
            // Dropping a failed transaction already schedules rollback; explicitly
            // await it so lock-hold measurements cover release as well.
            if let Err(rollback) = transaction.rollback().await {
                tracing::debug!(
                    reason = %rollback,
                    "Could not confirm failed installation rollback"
                );
            }
            Err(error)
        }
    };

    let outcome = match &result {
        Ok(Outcome::Installed { .. }) => "installed",
        Ok(_) => "unchanged",
        Err(_) => "failed",
    };
    metrics::histogram!("clacks_sync_baseline_lock_hold_seconds", "outcome" => outcome)
        .record(holding.elapsed().as_secs_f64());

    result
}

/// Read, prove and persist one conversion while its account remains locked.
async fn install_locked(
    connection: &mut PgConnection,
    owner: AccountId,
) -> anyhow::Result<Outcome> {
    let current = account::load(connection, owner).await?;
    let membership = crate::enrollment::membership::snapshot(connection, &current).await?;
    let interpretation = account::interpretation(connection, owner).await?;

    let mut heads = Vec::new();
    for row in sqlx::query_file!("sql/retention/heads.sql", owner.to_string())
        .fetch_all(&mut *connection)
        .await?
    {
        heads.push(Head {
            requirement: RecordRequirement {
                key: RecordKey {
                    account: owner,
                    id: payload::record_id(&row.kind, &row.record_id)?,
                },
                stamp: VersionStamp::new(
                    Position {
                        timeline: row.timeline_id.parse()?,
                        revision: row.revision.try_into()?,
                    },
                    row.baseline_id.parse()?,
                    serde_json::from_str(&row.body)?,
                )?,
                deleted: row.deleted,
            },
            payload_id: row.payload_id,
            envelope_id: row.envelope_id,
        });
    }

    metrics::histogram!("clacks_sync_baseline_contexts_examined").record(heads.len() as f64);

    let descriptors: Vec<_> = heads.iter().map(|head| head.requirement.clone()).collect();
    let Some(plan) = plan_installation(
        BaselineId::new(),
        &current,
        &membership,
        &descriptors,
        &interpretation.view(),
    )?
    else {
        return Ok(Outcome::Unchanged);
    };

    lineage::insert_baseline(connection, &plan.baseline).await?;
    for conversion in &plan.conversions {
        let head = heads
            .iter()
            .find(|head| head.requirement == conversion.original)
            .ok_or_else(|| anyhow::anyhow!("projection has no current envelope"))?;

        let descriptor = &conversion.projected;
        let key = descriptor.key;
        let account = owner.to_string();
        let kind = payload::kind(key.id);
        let id = key.id.to_string();

        let stamp = metadata::clock::insert_stamp(connection, key, &descriptor.stamp).await?;
        let envelope = sqlx::query_file!(
            "sql/persistence/client/envelope_insert.sql",
            account,
            kind,
            id,
            head.payload_id,
            stamp
        )
        .fetch_one(&mut *connection)
        .await?
        .envelope_id;

        sqlx::query_file!(
            "sql/synchronization/head_insert.sql",
            account,
            kind,
            id,
            envelope
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query_file!(
            "sql/retention/binding_project.sql",
            account,
            head.envelope_id,
            envelope
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query_file!(
            "sql/synchronization/journal_insert.sql",
            account,
            plan.account.head.timeline.to_string(),
            i64::try_from(plan.account.head.revision)?,
            kind,
            id,
            stamp,
            descriptor.deleted
        )
        .execute(&mut *connection)
        .await?;
    }

    account::save(connection, &plan.account).await?;

    Ok(Outcome::Installed {
        converted: plan.conversions.len().try_into()?,
    })
}
