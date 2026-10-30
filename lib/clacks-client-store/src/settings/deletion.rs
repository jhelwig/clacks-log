//! Preview equipment deletion and atomically repair referring station profiles.

use super::{EquipmentDeleteOutcome, EquipmentDeletionPreview, ProfileEquipmentReplacement};
use crate::{
    metadata::client,
    save::{self, SaveOutcome},
    store::ClientStore,
};
use clacks_model::ids::{AccountId, EquipmentConfigurationId};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use sqlx::SqliteConnection;
use std::collections::BTreeMap;

/// Read the complete working-profile set referencing one equipment identity.
pub(crate) async fn referring_profiles(
    connection: &mut SqliteConnection,
    account: AccountId,
    id: EquipmentConfigurationId,
) -> anyhow::Result<Vec<ClientRecord>> {
    let text = account.to_string();
    let equipment = id.to_string();
    let mut profiles = Vec::new();
    for row in sqlx::query_file!("sql/settings/referring_profiles.sql", text, equipment)
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account,
            id: RecordId::Profile(row.record_id.parse()?),
        };
        profiles.push(
            client::load(connection, key)
                .await?
                .ok_or_else(|| anyhow::anyhow!("referring profile disappeared in its snapshot"))?,
        );
    }
    Ok(profiles)
}

/// Assemble the exact review from a live target and one database snapshot.
async fn preview_in(
    connection: &mut SqliteConnection,
    account: AccountId,
    equipment: ClientRecord,
) -> anyhow::Result<EquipmentDeletionPreview> {
    let RecordId::Equipment(id) = equipment.key.id else {
        anyhow::bail!("equipment review requires an equipment record")
    };
    let view = super::read::load_in(connection, account).await?;
    let replacement_equipment = view
        .equipment
        .into_iter()
        .filter(|r| {
            r.key.id != equipment.key.id
                && matches!(
                    r.working_state(),
                    Some(RecordState::Live(RecordValue::Equipment(_)))
                )
        })
        .collect();
    Ok(EquipmentDeletionPreview {
        equipment,
        referring_profiles: referring_profiles(connection, account, id).await?,
        replacement_equipment,
    })
}

/// Prepare a consistent review without locking the database during user decisions.
pub(super) async fn preview(
    store: &ClientStore,
    id: EquipmentConfigurationId,
) -> anyhow::Result<EquipmentDeletionPreview> {
    let mut transaction = store.pool.begin().await?;
    let row = client::load(
        &mut transaction,
        RecordKey {
            account: store.account,
            id: RecordId::Equipment(id),
        },
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("equipment is not locally available"))?;
    anyhow::ensure!(
        matches!(
            row.working_state(),
            Some(RecordState::Live(RecordValue::Equipment(_)))
        ),
        "equipment has no live working configuration"
    );
    let result = preview_in(&mut transaction, store.account, row).await?;
    transaction.commit().await?;
    Ok(result)
}

/// Revalidate complete review inputs before committing any profile repair.
pub(super) async fn apply(
    store: &ClientStore,
    expected: &EquipmentDeletionPreview,
    replacements: &[ProfileEquipmentReplacement],
) -> anyhow::Result<EquipmentDeleteOutcome> {
    let key = expected.equipment.key;
    let RecordId::Equipment(target) = key.id else {
        anyhow::bail!("equipment deletion requires an equipment record")
    };
    anyhow::ensure!(
        key.account == store.account,
        "equipment belongs to another account"
    );
    let mut reviewed = BTreeMap::new();
    for row in &expected.referring_profiles {
        anyhow::ensure!(
            row.key.account == store.account
                && matches!(row.key.id, RecordId::Profile(_))
                && reviewed.insert(row.key, row).is_none(),
            "invalid or duplicate reviewed profile"
        );
    }
    let mut candidates = BTreeMap::new();
    for row in &expected.replacement_equipment {
        anyhow::ensure!(
            row.key.account == store.account
                && matches!(row.key.id, RecordId::Equipment(_))
                && row.key != key
                && candidates.insert(row.key, row).is_none(),
            "invalid or duplicate reviewed replacement"
        );
    }
    let mut transaction = store.write().await?;
    let current = client::load(&mut transaction, key).await?;
    let current = match current {
        Some(row)
            if matches!(
                row.working_state(),
                Some(RecordState::Live(RecordValue::Equipment(_)))
            ) =>
        {
            row
        }
        current => return Ok(EquipmentDeleteOutcome::Unavailable { current }),
    };
    let fresh = preview_in(&mut transaction, store.account, current).await?;
    let actual: BTreeMap<_, _> = fresh
        .referring_profiles
        .iter()
        .map(|r| (r.key, r))
        .collect();
    if fresh.equipment != expected.equipment {
        tracing::info!(event = "equipment.deletion_review_changed", account_id = %store.account, equipment_id = %target, reason = "target_changed");
        return Ok(EquipmentDeleteOutcome::Stale(fresh));
    }
    if actual != reviewed {
        tracing::info!(event = "equipment.deletion_review_changed", account_id = %store.account, equipment_id = %target, reason = "referrers_changed");
        return Ok(EquipmentDeleteOutcome::Stale(fresh));
    }
    let mut choices = BTreeMap::new();
    for choice in replacements {
        let profile_key = RecordKey {
            account: store.account,
            id: RecordId::Profile(choice.profile),
        };
        anyhow::ensure!(
            reviewed.contains_key(&profile_key)
                && choices.insert(profile_key, choice.equipment).is_none(),
            "unexpected or duplicate profile replacement choice"
        );
        if let Some(id) = choice.equipment {
            anyhow::ensure!(id != target, "deleted equipment cannot replace itself");
            let replacement_key = RecordKey {
                account: store.account,
                id: RecordId::Equipment(id),
            };
            let expected_replacement = candidates.get(&replacement_key).ok_or_else(|| {
                anyhow::anyhow!("replacement was not reviewed; refresh the preview")
            })?;
            if client::load(&mut transaction, replacement_key)
                .await?
                .as_ref()
                != Some(*expected_replacement)
            {
                tracing::info!(event = "equipment.deletion_review_changed", account_id = %store.account, equipment_id = %target, replacement_id = %id, reason = "replacement_changed");
                return Ok(EquipmentDeleteOutcome::Stale(fresh));
            }
            anyhow::ensure!(
                matches!(
                    expected_replacement.working_state(),
                    Some(RecordState::Live(RecordValue::Equipment(_)))
                ),
                "replacement has no live configuration"
            );
        }
    }
    anyhow::ensure!(
        choices.len() == reviewed.len(),
        "every referring profile requires an explicit replacement or removal"
    );
    for row in &fresh.referring_profiles {
        let Some(RecordState::Live(RecordValue::Profile(profile))) = row.working_state() else {
            anyhow::bail!("reviewed profile has no live working value")
        };
        let mut next = (**profile).clone().into_fields();
        next.default_equipment = *choices
            .get(&row.key)
            .ok_or_else(|| anyhow::anyhow!("profile choice is missing"))?;
        let outcome = save::save_in(
            &mut transaction,
            row.key,
            Some(row),
            RecordState::Live(RecordValue::Profile(Box::new(next.try_into()?))),
        )
        .await?;
        anyhow::ensure!(
            matches!(outcome, SaveOutcome::Saved(_)),
            "profile changed within deletion transaction"
        );
    }
    let outcome = save::save_in(
        &mut transaction,
        key,
        Some(&fresh.equipment),
        RecordState::Deleted,
    )
    .await?;
    anyhow::ensure!(
        matches!(outcome, SaveOutcome::Saved(_)),
        "equipment changed within deletion transaction"
    );
    let repaired = u64::try_from(fresh.referring_profiles.len())?;
    transaction.commit().await?;
    tracing::info!(event = "equipment.deleted", account_id = %store.account, equipment_id = %target, repaired_profiles = repaired);
    metrics::counter!("clacks_client_equipment_mutations_total", "change" => "delete").increment(1);
    if repaired > 0 {
        metrics::counter!("clacks_client_profile_mutations_total", "change" => "update")
            .increment(repaired);
    }
    Ok(EquipmentDeleteOutcome::Applied)
}
