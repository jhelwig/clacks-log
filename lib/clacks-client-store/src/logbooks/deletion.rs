//! Preview the impact of deleting a logbook tag and atomically apply the deletion.
//!
//! Deletion applies the chosen QSO changes and any selected-view fallback in
//! the same transaction.

use super::selection::{self, SelectedView};
use crate::{
    metadata::client,
    save::{SaveOutcome, save_in},
    store::ClientStore,
};
use clacks_model::{
    ids::TagId,
    tag::{TagDeletionChoice, TagDeletionPlan, plan_tag_deletion},
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use sqlx::SqliteConnection;

/// Counts of active QSOs affected by deleting a logbook tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagDeletionCounts {
    /// Contacts associated with this tag.
    pub affected: usize,
    /// Contacts also associated with other tags.
    pub also_tagged_elsewhere: usize,
    /// Contacts associated only with this tag.
    pub unique: usize,
}

/// Tag and QSO records to review before confirming deletion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagDeletionPreview {
    /// Tag record loaded for this deletion preview.
    pub tag: ClientRecord,
    /// One complete client record for each active QSO carrying this tag.
    pub members: Vec<ClientRecord>,
    /// Precomputed impact counts for the confirmation dialog.
    pub counts: TagDeletionCounts,
}

/// Result of deleting a logbook tag against a previously reviewed snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagDeleteOutcome {
    /// The tag deletion, chosen QSO changes and any view fallback committed together.
    Applied {
        /// QSOs marked deleted and memberships removed by this operation.
        plan: TagDeletionPlan,
        /// Authoritative local view after commit.
        selected_view: SelectedView,
        /// Deleted tag that caused a matching selected-view fallback.
        fallback_from: Option<TagId>,
    },
    /// The tag or its QSO records changed; nothing was deleted.
    Stale {
        /// Current tag record, if still available.
        current: Option<Box<ClientRecord>>,
    },
}

/// Load a deletion preview for one live tag and its active QSOs.
///
/// Pass the returned snapshot to [`delete_tag`] when confirming. Previewing does
/// not select the tag or change any record; missing or deleted tags are errors.
#[tracing::instrument(skip_all, name = "client_store.tag.preview", level = "debug", fields(account_id = %store.account, tag_id = %id))]
pub async fn preview(store: &ClientStore, id: TagId) -> anyhow::Result<TagDeletionPreview> {
    let mut transaction = store.pool.begin().await?;
    let key = RecordKey {
        account: store.account,
        id: RecordId::Tag(id),
    };
    let tag = client::load(&mut transaction, key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("logbook is unavailable"))?;
    anyhow::ensure!(
        matches!(
            tag.working_state(),
            Some(RecordState::Live(RecordValue::Tag(_)))
        ),
        "logbook is not live"
    );
    let members = members_in(&mut transaction, key).await?;
    let qsos = member_qsos(&members)?;
    let plan = plan_tag_deletion(id, TagDeletionChoice::DeleteUnique, &qsos);
    let counts = TagDeletionCounts {
        affected: plan.affected.len(),
        also_tagged_elsewhere: plan.also_tagged_elsewhere,
        unique: plan.mark_deleted.len(),
    };
    transaction.commit().await?;
    Ok(TagDeletionPreview {
        tag,
        members,
        counts,
    })
}

/// Load one complete client record per active QSO carrying the tag.
async fn members_in(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<Vec<ClientRecord>> {
    let account = key.account.to_string();
    let tag_text = key.id.to_string();
    let mut members = Vec::new();
    for row in sqlx::query_file!("sql/logbooks/members.sql", account, tag_text)
        .fetch_all(&mut *connection)
        .await?
    {
        let member_key = RecordKey {
            account: key.account,
            id: RecordId::Qso(row.record_id.parse()?),
        };
        members.push(
            client::load(connection, member_key)
                .await?
                .ok_or_else(|| anyhow::anyhow!("tag member lost its client row"))?,
        );
    }
    Ok(members)
}

/// Copy the live QSO values from their client records.
fn member_qsos(members: &[ClientRecord]) -> anyhow::Result<Vec<clacks_model::contact::Qso>> {
    members
        .iter()
        .map(|row| {
            let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
                anyhow::bail!("tag member has no live QSO");
            };
            Ok((**qso).clone())
        })
        .collect()
}

/// Delete the reviewed tag and atomically apply the chosen QSO changes.
///
/// Returns [`TagDeleteOutcome::Stale`] without changing records if the tag or any
/// member record changed after the preview. If the tag is selected, the same
/// transaction switches the view to All contacts.
#[tracing::instrument(skip_all, name = "client_store.tag.delete", fields(account_id = %store.account, deletion_choice = ?choice))]
pub async fn delete_tag(
    store: &ClientStore,
    expected: &TagDeletionPreview,
    choice: clacks_model::tag::TagDeletionChoice,
) -> anyhow::Result<TagDeleteOutcome> {
    let expected_tag = &expected.tag;
    let expected_members = &expected.members;
    anyhow::ensure!(
        expected_tag.key.account == store.account,
        crate::error::StoreError::WrongAccount {
            expected: store.account,
            actual: expected_tag.key.account
        }
    );
    let RecordId::Tag(tag) = expected_tag.key.id else {
        anyhow::bail!("tag deletion requires a tag key")
    };
    let mut transaction = store.write().await?;
    let selected_before = selection::load_in(&mut transaction).await?;
    let current = client::load(&mut transaction, expected_tag.key).await?;
    if current.as_ref() != Some(expected_tag) {
        tracing::debug!(event = "tag.deletion_stale", account_id = %store.account, tag_id = %tag, reason = "tag_changed");
        return Ok(TagDeleteOutcome::Stale {
            current: current.map(Box::new),
        });
    }
    anyhow::ensure!(
        matches!(
            expected_tag.working_state(),
            Some(RecordState::Live(RecordValue::Tag(_)))
        ),
        "tag has no live working value"
    );
    let members = members_in(&mut transaction, expected_tag.key).await?;
    let members: std::collections::BTreeMap<_, _> =
        members.into_iter().map(|row| (row.key, row)).collect();
    let mut expected = std::collections::BTreeMap::new();
    for row in expected_members {
        anyhow::ensure!(
            row.key.account == store.account && expected.insert(row.key, row.clone()).is_none(),
            "invalid or duplicate deletion-preview member"
        );
    }
    if members != expected {
        tracing::debug!(event = "tag.deletion_stale", account_id = %store.account, tag_id = %tag, reason = "members_changed");
        return Ok(TagDeleteOutcome::Stale {
            current: current.map(Box::new),
        });
    }
    let mut qsos = Vec::new();
    for row in members.values() {
        let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
            anyhow::bail!("tag member has no live QSO")
        };
        qsos.push((**qso).clone());
    }
    let plan = plan_tag_deletion(tag, choice, &qsos);
    let mut deleted = 0;
    let mut updated = 0;
    for row in members.values() {
        let RecordId::Qso(id) = row.key.id else {
            anyhow::bail!("invalid tag member kind")
        };
        let next = if plan.mark_deleted.contains(&id) {
            RecordState::Deleted
        } else {
            let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
                anyhow::bail!("tag member has no live QSO")
            };
            let mut qso = qso.clone();
            qso.tags.remove(&tag);
            RecordState::Live(RecordValue::Qso(qso))
        };
        match save_in(&mut transaction, row.key, Some(row), next).await? {
            SaveOutcome::Saved(_) => {
                if plan.mark_deleted.contains(&id) {
                    deleted += 1;
                } else {
                    updated += 1;
                }
            }
            SaveOutcome::Unchanged(_) => {}
            SaveOutcome::Stale { .. } => {
                anyhow::bail!("tag deletion member changed within its transaction")
            }
        }
    }
    anyhow::ensure!(
        !matches!(
            save_in(
                &mut transaction,
                expected_tag.key,
                Some(expected_tag),
                RecordState::Deleted
            )
            .await?,
            SaveOutcome::Stale { .. }
        ),
        "tag changed within deletion transaction"
    );
    let selected_view = selection::load_in(&mut transaction).await?;
    let fallback_from = (selected_before == SelectedView::Logbook(tag)).then_some(tag);
    transaction.commit().await?;
    if deleted > 0 {
        metrics::counter!("clacks_client_qso_mutations_total", "change" => "delete")
            .increment(deleted);
    }
    if updated > 0 {
        metrics::counter!("clacks_client_qso_mutations_total", "change" => "update")
            .increment(updated);
    }
    tracing::info!(event = "tag.deleted", account_id = %store.account, tag_id = %tag, deleted_contacts = deleted, changed_memberships = updated, fallback_from = ?fallback_from);
    metrics::counter!("clacks_client_tag_mutations_total", "change" => "delete").increment(1);
    Ok(TagDeleteOutcome::Applied {
        plan,
        selected_view,
        fallback_from,
    })
}
