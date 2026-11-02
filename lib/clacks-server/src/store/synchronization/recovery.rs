//! Guarded installation of previously accepted state and exceptional history boundaries.

use super::{context, install, install_accepted};
use crate::{
    enrollment::AuthorizedClient,
    store::{
        AccountStore,
        metadata::{self, account, lineage},
    },
};
use clacks_sync::{
    account::{Baseline, InstalledCheckpoint},
    client::OriginalBasis,
    lineage::Lineage,
    record::AcceptedRecord,
    recovery::{RecoveredDecision, RecoveryBoundaryPlan},
};

/// Store a core-planned boundary and account state together under the existing account lock.
pub(super) async fn boundary(
    connection: &mut sqlx::PgConnection,
    plan: &RecoveryBoundaryPlan,
    mut ancestry: Lineage,
) -> anyhow::Result<()> {
    ancestry.add_boundary(plan.boundary.clone())?;
    let id = lineage::insert_lineage(connection, &ancestry).await?;
    sqlx::query_file!(
        "sql/metadata/lineage_update.sql",
        plan.next_account.account.to_string(),
        id
    )
    .execute(&mut *connection)
    .await?;
    account::save(connection, &plan.next_account).await
}

/// Incorporate genuine retained interpretation without replacing the server's current history.
///
/// A witnessed revision regression forks the restored timeline once. Unknown ordering remains
/// explicit for the existing recovery planner; this does not claim universal restore detection.
pub(crate) async fn negotiate(
    store: &AccountStore,
    client: &AuthorizedClient,
    remembered: Option<&InstalledCheckpoint>,
    baselines: &[Baseline],
    ancestry: Option<&Lineage>,
) -> anyhow::Result<()> {
    let mut transaction = store.pool.begin().await?;
    let waiting = std::time::Instant::now();
    sqlx::query_file!("sql/account_lock.sql", client.account.to_string())
        .fetch_one(&mut *transaction)
        .await?;
    metrics::histogram!(crate::store::synchronization::SYNC_NEGOTIATION_LOCK_WAIT_SECONDS_METRIC)
        .record(waiting.elapsed().as_secs_f64());

    crate::oauth::resource::check_current(&mut transaction, client).await?;
    let current = account::load(&mut transaction, client.account).await?;
    let mut interpretation = account::interpretation(&mut transaction, client.account).await?;
    for value in baselines {
        anyhow::ensure!(value.account == client.account, "baseline crosses account");
        lineage::insert_baseline(&mut transaction, value).await?;
    }
    if let Some(ancestry) = ancestry {
        anyhow::ensure!(
            ancestry.account() == client.account
                && ancestry.initial_timeline() == interpretation.lineage.initial_timeline(),
            "lineage crosses account or root"
        );
        for value in ancestry.boundaries() {
            interpretation.lineage.add_boundary(value.clone())?;
        }
        let id = lineage::insert_lineage(&mut transaction, &interpretation.lineage).await?;
        sqlx::query_file!(
            "sql/metadata/lineage_update.sql",
            client.account.to_string(),
            id
        )
        .execute(&mut *transaction)
        .await?;
    }
    let mut forked = false;
    if let Some(remembered) = remembered {
        anyhow::ensure!(
            remembered.account == client.account,
            "checkpoint crosses account"
        );
        let check = clacks_sync::recovery::check_history(
            &clacks_sync::recovery::HistoryObservation {
                account: client.account,
                advertised: current.head,
                remembered: remembered.coverage.clone(),
            },
            &interpretation.view(),
        )?;
        if matches!(
            check,
            clacks_sync::recovery::HistoryCheck::Regression { .. }
        ) {
            let membership =
                crate::enrollment::membership::snapshot(&mut transaction, &current).await?;
            let plan = clacks_sync::recovery::plan_recovery_fork(
                clacks_model::ids::TimelineId::new(),
                &current,
                &membership,
                &current.installed,
            )?;
            boundary(&mut transaction, &plan, interpretation.lineage).await?;
            forked = true;
        }
    }
    transaction.commit().await?;
    if forked {
        let _ = store.changes.send(client.account);
    }
    Ok(())
}

/// Return an already accepted envelope through current carrier authority, preserving its stamp.
///
/// Exact support is retained as accepted evidence; it does not authorize unpublished edits by
/// its original writer. A review result is returned to the client, never queued on the server.
#[tracing::instrument(skip_all, name="server.sync.recover", fields(account_id=%client.account, actor_id=%client.actor))]
pub(crate) async fn install_returned(
    connection: &mut sqlx::PgConnection,
    client: &AuthorizedClient,
    incoming: &AcceptedRecord,
    original: Option<&OriginalBasis>,
) -> anyhow::Result<RecoveredDecision> {
    anyhow::ensure!(
        incoming.key().account == client.account,
        "accepted return crosses account"
    );
    let loaded = context::load(connection, incoming.key(), incoming.state(), &[]).await?;
    let decision = clacks_sync::recovery::plan_recovered_install(
        incoming,
        original,
        client.actor,
        &loaded.input(),
        &loaded.interpretation.view(),
    )?;
    if matches!(
        decision,
        RecoveredDecision::Install { .. } | RecoveredDecision::Reconciled(_)
    ) && let Some(clacks_sync::acceptance::RejectionReason::Referenced {
        referrers, total, ..
    }) = super::referenced(connection, incoming.key(), incoming.state()).await?
    {
        return Ok(RecoveredDecision::Review(
            clacks_sync::client::ReviewInput {
                original: original.cloned().unwrap_or(OriginalBasis::Unavailable {
                    key: incoming.key(),
                    reason: clacks_sync::error::UnknownReason::MissingOriginal,
                }),
                local: incoming.state().clone(),
                shared: loaded.current,
                retained: vec![incoming.clone()],
                reason: clacks_sync::client::ReviewReason::Referenced { referrers, total },
            },
        ));
    }
    // Retaining an envelope is not installing a head or certifying a complete prefix.
    metadata::accepted::insert_envelope(connection, incoming).await?;
    match &decision {
        RecoveredDecision::Install {
            envelope,
            journal,
            next_account,
            ..
        } => {
            install_accepted(connection, envelope, journal, next_account).await?;
        }
        RecoveredDecision::Reconciled(commit) => install(connection, commit).await?,
        _ => {}
    }
    Ok(decision)
}
