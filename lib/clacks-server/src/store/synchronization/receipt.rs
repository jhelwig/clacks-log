//! Durable returned facts, incomplete support and atomic coupled recovery effects.

use super::{context, recovery};
use crate::{
    enrollment::AuthorizedClient,
    store::{
        AccountStore,
        metadata::{self, account},
        payload,
    },
};
use clacks_sync::{
    record::{AcceptedRecord, Observation, RecordKey, RecordState, RecordValue},
    recovery::RecoveredDecision,
    service::operation::CoupledRequirement,
    transfer::discovery::DiscoveryPage,
};
use sqlx::{Connection, PgConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Committed returned-page effects; facts and staged input do not imply complete account coverage.
pub(crate) struct Received {
    /// Values durably represented by accepted heads or retained alternatives.
    pub(crate) represented_count: usize,
    /// Values still waiting for required support or safe coupled effects.
    pub(crate) staged: Vec<AcceptedRecord>,
    /// Units durably represented as a whole.
    pub(crate) completed_units: BTreeSet<clacks_model::ids::OperationId>,
    /// Actual client-owned comparisons requiring explicit choice.
    pub(crate) reviews: Vec<clacks_sync::client::ReviewInput>,
}

/// Retain an immutable coupled descriptor, rejecting a reused operation ID with different members.
async fn unit(
    connection: &mut PgConnection,
    owner: clacks_model::ids::AccountId,
    unit: &CoupledRequirement,
) -> anyhow::Result<()> {
    let existing = sqlx::query_file!(
        "sql/discovery/members.sql",
        owner.to_string(),
        unit.id.to_string()
    )
    .fetch_all(&mut *connection)
    .await?;
    if !existing.is_empty() {
        let mut members = Vec::new();
        for member in existing {
            let key = RecordKey {
                account: owner,
                id: payload::record_id(&member.kind, &member.record_id)?,
            };
            members.push(clacks_sync::service::VersionReference {
                key,
                stamp: metadata::clock::load_stamp(connection, key, member.stamp_id).await?,
                deleted: member.deleted,
            });
        }
        anyhow::ensure!(
            members == unit.members,
            "coupled operation identity has contradictory effects"
        );
        return Ok(());
    }
    sqlx::query_file!(
        "sql/recovery/unit_insert.sql",
        owner.to_string(),
        unit.id.to_string()
    )
    .execute(&mut *connection)
    .await?;
    for (ordinal, member) in unit.members.iter().enumerate() {
        anyhow::ensure!(
            member.key.account == owner,
            "coupled member crosses account"
        );
        let stamp = metadata::clock::insert_stamp(connection, member.key, &member.stamp).await?;
        sqlx::query_file!(
            "sql/recovery/member_insert.sql",
            owner.to_string(),
            unit.id.to_string(),
            i64::try_from(ordinal)?,
            payload::kind(member.key.id),
            member.key.id.to_string(),
            stamp,
            member.deleted
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Preserve a complete page and apply eligible native recovery units under one account transaction.
#[tracing::instrument(skip_all, name="server.sync.receive", fields(account_id=%client.account, actor_id=%client.actor, page=page.sequence))]
pub(crate) async fn receive(
    store: &AccountStore,
    client: &AuthorizedClient,
    page: &DiscoveryPage,
) -> anyhow::Result<Received> {
    let mut transaction = store.pool.begin().await?;
    let waiting = std::time::Instant::now();
    sqlx::query_file!("sql/account_lock.sql", client.account.to_string())
        .fetch_one(&mut *transaction)
        .await?;
    metrics::histogram!(crate::store::synchronization::SYNC_RETURN_LOCK_WAIT_SECONDS_METRIC)
        .record(waiting.elapsed().as_secs_f64());

    crate::oauth::resource::check_current(&mut transaction, client).await?;
    let before = account::load(&mut transaction, client.account).await?;
    for baseline in &page.baselines {
        anyhow::ensure!(
            baseline.account == client.account,
            "baseline crosses account"
        );
        metadata::lineage::insert_baseline(&mut transaction, baseline).await?;
    }
    let mut interpretation = account::interpretation(&mut transaction, client.account).await?;
    let old_ancestry = interpretation.lineage.clone();
    for boundary in &page.boundaries {
        interpretation.lineage.add_boundary(boundary.clone())?;
    }
    if old_ancestry != interpretation.lineage {
        let id =
            metadata::lineage::insert_lineage(&mut transaction, &interpretation.lineage).await?;
        sqlx::query_file!(
            "sql/metadata/lineage_update.sql",
            client.account.to_string(),
            id
        )
        .execute(&mut *transaction)
        .await?;
    }
    for value in page.accepted.iter().chain(&page.support) {
        anyhow::ensure!(
            value.key().account == client.account,
            "returned value crosses account"
        );
        let id = metadata::accepted::insert_envelope(&mut transaction, value).await?;
        sqlx::query_file!(
            "sql/recovery/stage.sql",
            client.account.to_string(),
            payload::kind(value.key().id),
            value.key().id.to_string(),
            id
        )
        .execute(&mut *transaction)
        .await?;
    }
    for descriptor in &page.units {
        unit(&mut transaction, client.account, descriptor).await?;
    }
    let mut current = account::load(&mut transaction, client.account).await?;
    if let Some(witness) = page.accepted.iter().chain(&page.support).find(|value| {
        let position = value.stamp.accepted_at();
        position.timeline == current.head.timeline && position.revision > current.head.revision
    }) {
        anyhow::ensure!(
            witness.key().account == client.account,
            "accepted witness crosses account"
        );
        // An accepted position proves regression, not completion of its whole prefix.
        let membership =
            crate::enrollment::membership::snapshot(&mut transaction, &current).await?;
        let plan = clacks_sync::recovery::plan_recovery_fork(
            clacks_model::ids::TimelineId::new(),
            &current,
            &membership,
            &current.installed,
        )?;
        recovery::boundary(&mut transaction, &plan, interpretation.lineage.clone()).await?;
        current = plan.next_account;
        interpretation = account::interpretation(&mut transaction, client.account).await?;
    }
    let mut incoming = Vec::new();
    for row in sqlx::query_file!("sql/recovery/staged.sql", client.account.to_string())
        .fetch_all(&mut *transaction)
        .await?
    {
        let key = RecordKey {
            account: client.account,
            id: payload::record_id(&row.kind, &row.record_id)?,
        };
        incoming
            .push(metadata::accepted::load_envelope(&mut transaction, key, row.envelope_id).await?);
    }
    let mut available = incoming.clone();
    let mut support = Vec::new();
    let units = crate::store::discovery::support::load(
        &mut transaction,
        &current,
        &mut available,
        &mut support,
        false,
    )
    .await?;
    available.extend(support);
    let mut heads = BTreeMap::new();
    for value in &available {
        let loaded = context::load(&mut transaction, value.key(), value.state(), &[]).await?;
        // Acceptance references include exact historical facts. Only actual current
        // observations belong in the semantic-head input to readiness.
        for key in
            std::iter::once(value.key()).chain(loaded.references.iter().map(Observation::key))
        {
            if let std::collections::btree_map::Entry::Vacant(entry) = heads.entry(key)
                && let Observation::Accepted(value) =
                    context::observation(&mut transaction, &current, key).await?
            {
                entry.insert(value);
            }
        }
    }
    let batch = clacks_sync::transfer::readiness::plan_readiness(
        &clacks_sync::transfer::readiness::ReadinessInput {
            account: client.account,
            candidates: &available,
            heads: &heads.into_values().collect::<Vec<_>>(),
            support: &[],
            units: &units,
        },
        &interpretation.view(),
    )?;
    let mut ready = batch.ready;
    ready.retain(|value| incoming.contains(value));
    let mut result = Received {
        represented_count: 0,
        staged: batch
            .staged
            .into_iter()
            .filter(|value| incoming.contains(value))
            .collect(),
        completed_units: BTreeSet::new(),
        reviews: vec![],
    };
    while let Some(first) = ready.first() {
        // Overlapping units form one atomic recovery group; no partial effect escapes it.
        let mut keys: BTreeSet<_> = [first.key()].into();
        loop {
            let before = keys.len();
            for unit in &units {
                if unit.members.iter().any(|member| keys.contains(&member.key)) {
                    keys.extend(unit.members.iter().map(|member| member.key));
                }
            }
            if before == keys.len() {
                break;
            }
        }
        let mut group = Vec::new();
        ready.retain(|value| {
            if keys.contains(&value.key()) {
                group.push(value.clone());
                false
            } else {
                true
            }
        });
        let mut nested = transaction.begin().await?;
        let mut reviews = Vec::new();
        let mut deferred = false;
        let mut pending = group.clone();
        while !pending.is_empty() {
            let index = pending.iter().position(|value| {
                let mut dependencies =
                    clacks_sync::acceptance::required_references(value.key(), value.state());
                if let RecordState::Live(RecordValue::Service(service)) = value.state() {
                    dependencies.extend(clacks_sync::service::dependency::required_keys(
                        client.account,
                        service,
                    ));
                }
                dependencies
                    .iter()
                    .all(|key| !pending.iter().any(|other| other.key() == *key))
            });
            let Some(index) = index else {
                deferred = true;
                break;
            };
            let value = pending.remove(index);
            match recovery::install_returned(&mut nested, client, &value, None).await? {
                RecoveredDecision::Review(input) => reviews.push(input),
                RecoveredDecision::DeferredService { .. } => {
                    deferred = true;
                    break;
                }
                _ => {}
            }
        }
        if deferred || !reviews.is_empty() {
            nested.rollback().await?;
        } else {
            nested.commit().await?;
        }
        if deferred {
            result.staged.extend(group);
            continue;
        }
        for value in &group {
            let id = metadata::accepted::insert_envelope(&mut transaction, value).await?;
            if !reviews.is_empty() {
                sqlx::query_file!(
                    "sql/recovery/retain.sql",
                    client.account.to_string(),
                    payload::kind(value.key().id),
                    value.key().id.to_string(),
                    id
                )
                .execute(&mut *transaction)
                .await?;
            }
            sqlx::query_file!("sql/recovery/unstage.sql", client.account.to_string(), id)
                .execute(&mut *transaction)
                .await?;
        }
        result.represented_count += group.len();
        let installed = reviews.is_empty();
        result.reviews.extend(reviews);
        for unit in units.iter().filter(|_| installed) {
            if unit.members.iter().all(|member| keys.contains(&member.key)) {
                result.completed_units.insert(unit.id);
            }
        }
    }
    let changed = account::load(&mut transaction, client.account).await? != before;
    transaction.commit().await?;
    if changed {
        let _ = store.changes.send(client.account);
    }
    tracing::info!(event="sync.returned_page.committed", account_id=%client.account, represented_count=result.represented_count, staged_count=result.staged.len(), review_count=result.reviews.len());
    metrics::counter!("clacks_server_sync_received_pages_total").increment(1);
    Ok(result)
}

/// Verify returned complete prefixes from actual installed versions before adding recovered ancestry.
pub(crate) async fn complete_return(
    store: &AccountStore,
    client: &AuthorizedClient,
    receipt: &clacks_sync::transfer::receipt::ReceiptState,
    end: &clacks_sync::transfer::discovery::DiscoveryEnd,
) -> anyhow::Result<bool> {
    if !receipt.staged.is_empty() {
        return Ok(false);
    }
    let mut transaction = store.pool.begin().await?;
    let waiting = std::time::Instant::now();
    sqlx::query_file!("sql/account_lock.sql", client.account.to_string())
        .fetch_one(&mut *transaction)
        .await?;
    metrics::histogram!(
        crate::store::synchronization::SYNC_RETURN_COMPLETE_LOCK_WAIT_SECONDS_METRIC
    )
    .record(waiting.elapsed().as_secs_f64());

    crate::oauth::resource::check_current(&mut transaction, client).await?;
    let current = account::load(&mut transaction, client.account).await?;
    let interpretation = account::interpretation(&mut transaction, client.account).await?;
    let mut installed = Vec::new();
    for required in &receipt.records {
        installed.extend(installed_heads(&mut transaction, required.key).await?);
    }
    let completed = match clacks_sync::transfer::receipt::finish_receipt(
        receipt,
        end,
        &installed,
        None,
        &interpretation.view(),
    ) {
        Ok(value) => value,
        Err(error)
            if matches!(
                error.downcast_ref::<clacks_sync::error::MetadataError>(),
                Some(clacks_sync::error::MetadataError::Incomplete { .. })
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let completed = clacks_sync::account::InstalledCheckpoint {
        coverage: clacks_sync::transfer::discovery::target_coverage(
            &completed,
            &interpretation.view(),
        )?,
        ..completed
    };
    if completed.coverage.entries().all(|(timeline, revision)| {
        current
            .installed
            .covers(clacks_sync::record::Position { timeline, revision })
    }) {
        transaction.commit().await?;
        return Ok(true);
    }
    let members = crate::enrollment::membership::snapshot(&mut transaction, &current).await?;
    let mut represented = current.clone();
    represented.installed = represented.installed.join(&completed.coverage);
    let plan = clacks_sync::recovery::plan_recovered_coverage(
        &represented,
        &completed,
        &members,
        &interpretation.view(),
    )?;
    let mut ancestry = interpretation.lineage;
    ancestry.add_boundary(plan.boundary)?;
    let id = crate::store::metadata::lineage::insert_lineage(&mut transaction, &ancestry).await?;
    sqlx::query_file!(
        "sql/metadata/lineage_update.sql",
        client.account.to_string(),
        id
    )
    .execute(&mut *transaction)
    .await?;
    account::save(&mut transaction, &plan.next_account).await?;
    transaction.commit().await?;
    let _ = store.changes.send(client.account);
    Ok(true)
}

/// Reload accepted heads for completion proof, excluding unresolved alternatives and staging.
async fn installed_heads(
    connection: &mut sqlx::PgConnection,
    key: clacks_sync::record::RecordKey,
) -> anyhow::Result<Vec<clacks_sync::record::AcceptedRecord>> {
    let mut values = Vec::new();
    for row in sqlx::query_file!(
        "sql/recovery/installed.sql",
        key.account.to_string(),
        crate::store::payload::kind(key.id),
        key.id.to_string()
    )
    .fetch_all(&mut *connection)
    .await?
    {
        values.push(
            crate::store::metadata::accepted::load_envelope(connection, key, row.envelope_id)
                .await?,
        );
    }
    Ok(values)
}
