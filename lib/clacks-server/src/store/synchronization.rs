//! PostgreSQL transactions applying native acceptance and accepted-state recovery.

pub(crate) mod context;
pub(crate) mod receipt;
pub(crate) mod recovery;

use super::{
    metadata::{self, account, lineage},
    payload,
};
use clacks_model::ids::{AccountId, BaselineId, TimelineId};
use clacks_sync::{
    acceptance::{AcceptanceCommit, AcceptanceDecision, AcceptanceInput, ServerChange},
    account::{AccountMetadata, InstalledCheckpoint},
    client::VersionCondition,
    membership::MembershipSnapshot,
    record::{Observation, RecordKey, RecordState, RecordValue},
};
use sqlx::PgConnection;

/// Accept a frozen proposal under current account and credential authority.
///
/// The returned accepted value is committed, including its journal and index effects.
#[tracing::instrument(skip_all, name = "server.sync.accept", fields(account_id = %client.account, actor_id = %client.actor))]
pub(crate) async fn accept_pending(
    store: &super::AccountStore,
    client: &crate::enrollment::AuthorizedClient,
    change: &clacks_sync::client::PreparedChange,
) -> anyhow::Result<AcceptanceDecision> {
    let key = change.key();
    anyhow::ensure!(
        key.account == client.account && change.actor() == client.actor,
        "proposal does not belong to the authenticated account/actor"
    );

    let mut transaction = store.pool.begin().await?;
    let waiting = std::time::Instant::now();
    sqlx::query_file!("sql/account_lock.sql", client.account.to_string())
        .fetch_one(&mut *transaction)
        .await?;
    metrics::histogram!("clacks_sync_accept_lock_wait_seconds")
        .record(waiting.elapsed().as_secs_f64());

    crate::oauth::resource::check_current(&mut transaction, client).await?;
    let loaded =
        context::load(&mut transaction, key, change.state(), change.dependencies()).await?;
    let mut decision = clacks_sync::acceptance::plan_acceptance(
        change,
        &loaded.input(),
        &loaded.interpretation.view(),
    )?;
    if matches!(decision, AcceptanceDecision::Commit(_))
        && let Some(reason) = referenced(&mut transaction, key, change.state()).await?
    {
        decision = AcceptanceDecision::Rejected(reason);
    }
    if let AcceptanceDecision::Commit(commit) = &decision {
        install(&mut transaction, commit).await?;
    }
    transaction.commit().await?;
    match &decision {
        AcceptanceDecision::Commit(commit) => {
            let _ = store.changes.send(client.account);
            tracing::info!(event = "sync.accepted", account_id = %client.account, record_id = %key.id, native_changed = commit.native_changed);
            metrics::counter!("clacks_server_sync_records_accepted_total").increment(1);
        }
        AcceptanceDecision::Covered(_) => {
            tracing::debug!(event = "sync.proposal.covered", account_id = %client.account, record_id = %key.id)
        }
        AcceptanceDecision::Stale(_) => {
            tracing::debug!(event = "sync.proposal.stale", account_id = %client.account, record_id = %key.id)
        }
        AcceptanceDecision::Rejected(reason) => match reason {
            clacks_sync::acceptance::RejectionReason::ServiceCondition { .. } => {
                tracing::info!(event="sync.proposal.rejected", account_id=%client.account, reason="service_condition")
            }
            _ => {
                tracing::info!(event = "sync.proposal.rejected", account_id = %client.account, record_id = %key.id, reason = ?reason)
            }
        },
    }
    Ok(decision)
}

/// Provision initial synchronization metadata and ordinary accepted account definitions.
///
/// Call once from the transaction creating the account, before exposing that account.
#[tracing::instrument(skip_all, name = "server.account.initialize", level = "debug", fields(account_id = %owner))]
pub(crate) async fn initialize(
    connection: &mut PgConnection,
    owner: AccountId,
) -> anyhow::Result<()> {
    let (mut state, baseline) =
        AccountMetadata::initialize(owner, TimelineId::new(), BaselineId::new());
    lineage::insert_baseline(connection, &baseline).await?;
    let coverage = lineage::insert_coverage(connection, owner, &state.installed).await?;
    let lineage = lineage::insert_lineage(connection, &baseline.lineage).await?;
    let owner_text = owner.to_string();
    let timeline = state.head.timeline.to_string();
    let baseline_id = baseline.id.to_string();
    sqlx::query_file!(
        "sql/metadata/state_insert.sql",
        owner_text,
        timeline,
        0_i64,
        baseline_id,
        0_i64,
        coverage,
        lineage
    )
    .execute(&mut *connection)
    .await?;

    let definitions = clacks_model::defaults::account_definitions()?;
    let seeds = definitions
        .output_profiles
        .into_iter()
        .map(|value| RecordValue::OutputProfile(Box::new(value)))
        .chain(
            definitions
                .groups
                .into_iter()
                .map(|value| RecordValue::Group(Box::new(value))),
        );
    let membership = MembershipSnapshot {
        account: owner,
        generation: 0,
        clients: vec![],
    };
    let interpretation = account::interpretation(connection, owner).await?;
    for value in seeds {
        let key = RecordKey {
            account: owner,
            id: value.id(),
        };
        let current = Observation::Absent {
            key,
            checkpoint: checkpoint(&state),
        };
        let change = ServerChange {
            key,
            state: RecordState::Live(value),
            expected: VersionCondition::from_observation(&current)?,
            dependencies: vec![],
        };
        let input = AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &state,
            membership: &membership,
            current: &current,
            references: &[],
        };
        let decision = clacks_sync::acceptance::plan_server_acceptance(
            &change,
            &input,
            &interpretation.view(),
        )?;
        let AcceptanceDecision::Commit(commit) = decision else {
            anyhow::bail!("new account definition was not accepted");
        };
        install(connection, &commit).await?;
        state = commit.next_account;
    }
    Ok(())
}

/// Describe only the native account's actual complete installed prefixes.
pub(crate) fn checkpoint(account: &AccountMetadata) -> InstalledCheckpoint {
    InstalledCheckpoint {
        account: account.account,
        baseline: account.baseline,
        coverage: account.installed.clone(),
    }
}

/// Persist one native acceptance plan in the transaction that checked its conditions.
async fn install(connection: &mut PgConnection, commit: &AcceptanceCommit) -> anyhow::Result<()> {
    install_accepted(
        connection,
        &commit.accepted,
        &commit.journal,
        &commit.next_account,
    )
    .await
}

/// Persist the shared effects of native acceptance and original-stamp recovered installation.
async fn install_accepted(
    connection: &mut PgConnection,
    accepted: &clacks_sync::record::AcceptedRecord,
    journal: &clacks_sync::acceptance::JournalChange,
    next_account: &AccountMetadata,
) -> anyhow::Result<()> {
    let envelope = metadata::accepted::insert_envelope(connection, accepted).await?;
    let key = accepted.key();
    let owner = key.account.to_string();
    let kind = payload::kind(key.id);
    let id = key.id.to_string();
    let association = matches!(
        key.id,
        clacks_sync::record::RecordId::Service(
            clacks_model::service::evidence::ServiceRecordId::Association(_)
        )
    );
    if association {
        sqlx::query_file!("sql/synchronization/binding_remove.sql", owner, id)
            .execute(&mut *connection)
            .await?;
    }
    sqlx::query_file!(
        "sql/synchronization/head_insert.sql",
        owner,
        kind,
        id,
        envelope
    )
    .execute(&mut *connection)
    .await?;
    if association {
        sqlx::query_file!("sql/synchronization/binding_insert.sql", owner, id)
            .execute(&mut *connection)
            .await?;
    }
    let stamp = metadata::clock::insert_stamp(connection, key, &journal.requirement.stamp).await?;
    let timeline = journal.installed_at.timeline.to_string();
    let revision = i64::try_from(journal.installed_at.revision)?;
    sqlx::query_file!(
        "sql/synchronization/journal_insert.sql",
        owner,
        timeline,
        revision,
        kind,
        id,
        stamp,
        journal.requirement.deleted
    )
    .execute(&mut *connection)
    .await?;
    account::save(connection, next_account).await
}

/// Save this client's completed issued coverage without advancing another client's retention pin.
///
/// The connection must match this receipt to its issued discovery before calling this boundary.
pub(crate) async fn acknowledge(
    store: &super::AccountStore,
    client: &crate::enrollment::AuthorizedClient,
    completed: &InstalledCheckpoint,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        completed.account == client.account,
        "acknowledgment crosses account"
    );
    let mut transaction = store.pool.begin().await?;
    let waiting = std::time::Instant::now();
    sqlx::query_file!("sql/account_lock.sql", client.account.to_string())
        .fetch_one(&mut *transaction)
        .await?;
    metrics::histogram!("clacks_sync_acknowledgment_lock_wait_seconds")
        .record(waiting.elapsed().as_secs_f64());

    crate::oauth::resource::check_current(&mut transaction, client).await?;
    let current = account::load(&mut transaction, client.account).await?;
    let interpretation = account::interpretation(&mut transaction, client.account).await?;
    let coverage =
        clacks_sync::transfer::discovery::target_coverage(completed, &interpretation.view())?;
    anyhow::ensure!(
        coverage.entries().all(|(timeline, revision)| current
            .installed
            .covers(clacks_sync::record::Position { timeline, revision })),
        "acknowledgment exceeds current represented coverage"
    );
    let members = crate::enrollment::membership::snapshot(&mut transaction, &current).await?;
    let old = members
        .clients
        .iter()
        .find(|member| member.actor == client.actor)
        .and_then(|member| member.acknowledged.as_ref());
    let next = InstalledCheckpoint {
        account: completed.account,
        baseline: completed.baseline,
        coverage: old.map_or_else(
            || completed.coverage.clone(),
            |old| old.coverage.join(&completed.coverage),
        ),
    };
    let id = lineage::insert_checkpoint(&mut transaction, &next).await?;
    anyhow::ensure!(
        sqlx::query_file!(
            "sql/synchronization/acknowledge.sql",
            client.account.to_string(),
            client.actor.to_string(),
            id
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected()
            == 1,
        "client is no longer admitted"
    );
    transaction.commit().await?;
    Ok(())
}

/// Block tag/equipment deletion while current accepted records reference its identity.
pub(super) async fn referenced(
    connection: &mut PgConnection,
    key: RecordKey,
    state: &RecordState,
) -> anyhow::Result<Option<clacks_sync::acceptance::RejectionReason>> {
    if !matches!(
        key.id,
        clacks_sync::record::RecordId::Tag(_) | clacks_sync::record::RecordId::Equipment(_)
    ) || !matches!(state, RecordState::Deleted)
    {
        return Ok(None);
    }
    let rows = sqlx::query_file!(
        "sql/synchronization/referrers.sql",
        key.account.to_string(),
        key.id.to_string(),
        payload::kind(key.id)
    )
    .fetch_all(&mut *connection)
    .await?;
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let total = u64::try_from(first.total)?;
    let mut referrers = Vec::new();
    for row in rows {
        referrers.push(RecordKey {
            account: key.account,
            id: payload::record_id(&row.kind, &row.record_id)?,
        });
    }
    Ok(Some(clacks_sync::acceptance::RejectionReason::Referenced {
        key,
        referrers,
        total,
    }))
}
