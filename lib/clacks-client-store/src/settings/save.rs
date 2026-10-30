//! Save reusable station profiles and equipment configurations.

use super::ProfileSaveResult;
use crate::{
    metadata::client,
    save::{self, SaveOutcome},
    store::ClientStore,
};
use clacks_model::profile::{EquipmentConfiguration, SavedLocationProfile};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};

/// Reject a foreign or different editor basis before looking up its target.
fn check_basis(key: RecordKey, expected: Option<&ClientRecord>) -> anyhow::Result<()> {
    if let Some(expected) = expected {
        anyhow::ensure!(
            expected.key == key,
            "editor basis belongs to another record or account"
        );
    }
    Ok(())
}

/// Commit the profile's own edit basis without silently repairing an unavailable default.
pub(super) async fn profile(
    store: &ClientStore,
    expected: Option<&ClientRecord>,
    next: &SavedLocationProfile,
) -> anyhow::Result<ProfileSaveResult> {
    let key = RecordKey {
        account: store.account,
        id: RecordId::Profile(next.id()),
    };
    check_basis(key, expected)?;
    let mut transaction = store.write().await?;
    let outcome = save::save_in(
        &mut transaction,
        key,
        expected,
        RecordState::Live(RecordValue::Profile(Box::new(next.clone()))),
    )
    .await?;
    let mut missing_equipment = None;
    if !matches!(outcome, SaveOutcome::Stale { .. })
        && let Some(id) = next.default_equipment()
    {
        let equipment = client::load(
            &mut transaction,
            RecordKey {
                account: store.account,
                id: RecordId::Equipment(id),
            },
        )
        .await?;
        if !equipment.as_ref().is_some_and(|record| {
            matches!(
                record.working_state(),
                Some(RecordState::Live(RecordValue::Equipment(_)))
            )
        }) {
            missing_equipment = Some(id);
        }
    }
    transaction.commit().await?;
    if let SaveOutcome::Saved(_) = &outcome {
        let change = if expected
            .and_then(ClientRecord::working_state)
            .is_some_and(|s| matches!(s, RecordState::Live(_)))
        {
            "update"
        } else {
            "create"
        };
        tracing::info!(event = "profile.saved", account_id = %store.account, profile_id = %next.id(), missing_equipment = ?missing_equipment);
        metrics::counter!("clacks_client_profile_mutations_total", "change" => change).increment(1);
    }
    Ok(ProfileSaveResult {
        outcome,
        missing_equipment,
    })
}

/// Reuse the ordinary shared-save transaction for independent equipment values.
pub(super) async fn equipment(
    store: &ClientStore,
    expected: Option<&ClientRecord>,
    next: &EquipmentConfiguration,
) -> anyhow::Result<SaveOutcome> {
    let key = RecordKey {
        account: store.account,
        id: RecordId::Equipment(next.id()),
    };
    check_basis(key, expected)?;
    save::save(
        store,
        key,
        expected,
        RecordState::Live(RecordValue::Equipment(Box::new(next.clone()))),
    )
    .await
}
