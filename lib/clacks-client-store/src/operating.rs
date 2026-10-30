//! Client-local operating facts and independent optional saved-source selections.

pub(crate) mod selections;
mod values;

use crate::{rows::operating::OperatingStateRow, store::ClientStore};
use clacks_model::{
    ids::AccountId,
    profile::{CurrentOperatingValues, EquipmentConfiguration, SavedLocationProfile},
};
use sqlx::SqliteConnection;

/// Active copied facts and exact source baselines, independent of later shared edits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperatingState {
    /// Complete applied facts, absent until first setup.
    pub(crate) current: Option<CurrentOperatingValues>,
    /// Exact selected station profile, absent after manual station adjustment.
    pub(crate) selected_profile: Option<SavedLocationProfile>,
    /// Exact selected equipment configuration, absent after manual equipment adjustment.
    pub(crate) selected_equipment: Option<EquipmentConfiguration>,
}

impl OperatingState {
    /// Construct applied values together with the exact optional source baselines.
    ///
    /// A selected source must describe its copied component; manual adjustments
    /// use no selection for the affected component.
    pub fn new(
        current: Option<CurrentOperatingValues>,
        selected_profile: Option<SavedLocationProfile>,
        selected_equipment: Option<EquipmentConfiguration>,
    ) -> anyhow::Result<Self> {
        let state = Self {
            current,
            selected_profile,
            selected_equipment,
        };
        state.validate()?;
        Ok(state)
    }

    /// Borrow applied operating values, absent before first setup.
    pub fn current(&self) -> Option<&CurrentOperatingValues> {
        self.current.as_ref()
    }

    /// Borrow the exact selected profile baseline, independent of later shared edits.
    pub fn selected_profile(&self) -> Option<&SavedLocationProfile> {
        self.selected_profile.as_ref()
    }

    /// Borrow the exact selected equipment baseline, independent of later shared edits.
    pub fn selected_equipment(&self) -> Option<&EquipmentConfiguration> {
        self.selected_equipment.as_ref()
    }

    /// Apply adjusted facts, detaching only sources whose copied component changed.
    ///
    /// Operator and power changes preserve both selections. A detached source is
    /// never inferred again merely because later adjustments match its values.
    pub fn adjust(&mut self, current: CurrentOperatingValues) {
        if self
            .selected_profile
            .as_ref()
            .is_some_and(|p| p.station() != &current.station)
        {
            self.selected_profile = None;
        }
        if self
            .selected_equipment
            .as_ref()
            .is_some_and(|e| current.equipment.as_ref() != Some(e.values()))
        {
            self.selected_equipment = None;
        }
        self.current = Some(current);
    }

    /// Clear a source selection while retaining its copied operating facts.
    pub fn forget_source(&mut self, source: clacks_sync::record::RecordId) {
        if self
            .selected_profile
            .as_ref()
            .is_some_and(|p| source == clacks_sync::record::RecordId::Profile(p.id()))
        {
            self.selected_profile = None;
        }
        if self
            .selected_equipment
            .as_ref()
            .is_some_and(|e| source == clacks_sync::record::RecordId::Equipment(e.id()))
        {
            self.selected_equipment = None;
        }
    }

    /// Validate facts and ensure each selection describes its own copied component.
    fn validate(&self) -> anyhow::Result<()> {
        let Some(current) = &self.current else {
            anyhow::ensure!(
                self.selected_profile.is_none() && self.selected_equipment.is_none(),
                "source selections require current operating values"
            );
            return Ok(());
        };

        if let Some(profile) = &self.selected_profile {
            anyhow::ensure!(
                profile.station() == &current.station,
                "selected profile does not match copied station values"
            );
        }
        if let Some(equipment) = &self.selected_equipment {
            anyhow::ensure!(
                current.equipment.as_ref() == Some(equipment.values()),
                "selected configuration does not match copied equipment values"
            );
        }
        Ok(())
    }
}

/// Result of checking the caller's complete operating-state basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperatingSaveOutcome {
    /// Changed values or selections committed together.
    Saved(OperatingState),
    /// Values and selections already match; no mutation committed.
    Unchanged(OperatingState),
    /// Another operating edit intervened; preserve the caller's form for re-evaluation.
    Stale(OperatingState),
}

/// Restore facts and exact source copies using the caller's existing snapshot.
pub(crate) async fn load_in(
    connection: &mut SqliteConnection,
    account: AccountId,
) -> anyhow::Result<OperatingState> {
    let text = account.to_string();
    let Some(row) = sqlx::query_file_as!(OperatingStateRow, "sql/operating/state_select.sql", text)
        .fetch_optional(&mut *connection)
        .await?
    else {
        return Ok(OperatingState::default());
    };
    let current = match row.current_values {
        Some(id) => Some(values::load_value(connection, account, id).await?),
        None => None,
    };
    let sources = selections::load(connection, account, &row).await?;
    OperatingState::new(current, sources.profile, sources.equipment)
}

/// Write copied facts and independent baseline references in the caller's transaction.
async fn store_in(
    connection: &mut SqliteConnection,
    account: AccountId,
    state: &OperatingState,
) -> anyhow::Result<()> {
    let current_values = match &state.current {
        Some(value) => Some(values::insert_value(connection, account, value).await?),
        None => None,
    };
    let sources = selections::store(connection, account, state).await?;
    let loaded_profile = sources.profile.as_ref().map(|source| source.payload);
    let profile_record = sources
        .profile
        .as_ref()
        .map(|source| source.record.as_str());
    let loaded_equipment = sources.equipment.as_ref().map(|source| source.payload);
    let equipment_record = sources
        .equipment
        .as_ref()
        .map(|source| source.record.as_str());
    let account = account.to_string();
    sqlx::query_file!(
        "sql/operating/state_store.sql",
        account,
        current_values,
        loaded_profile,
        profile_record,
        loaded_equipment,
        equipment_record
    )
    .execute(connection)
    .await?;
    Ok(())
}

/// Read the applied context and its optional selections from one local snapshot.
pub async fn load(store: &ClientStore) -> anyhow::Result<OperatingState> {
    let mut transaction = store.pool.begin().await?;
    let state = load_in(&mut transaction, store.account).await?;
    transaction.commit().await?;
    Ok(state)
}

/// Apply validated copied facts and selections only if the active read basis still matches.
#[tracing::instrument(skip_all, name = "client_store.operating.save", fields(account_id = %store.account))]
pub async fn save(
    store: &ClientStore,
    expected: &OperatingState,
    next: &OperatingState,
) -> anyhow::Result<OperatingSaveOutcome> {
    let mut transaction = store.write().await?;
    let current = load_in(&mut transaction, store.account).await?;
    let deleted =
        selections::deleted_sources(&mut transaction, store.account, &[expected, &current, next])
            .await?;
    let mut expected = expected.clone();
    let mut comparable_current = current.clone();
    let mut next = next.clone();
    selections::forget(&mut expected, &deleted);
    selections::forget(&mut comparable_current, &deleted);
    selections::forget(&mut next, &deleted);
    if comparable_current != expected {
        tracing::info!(event = "operating.apply_deferred", account_id = %store.account, reason = "active_state_changed");
        return Ok(OperatingSaveOutcome::Stale(current));
    }
    if current == next {
        return Ok(OperatingSaveOutcome::Unchanged(current));
    }
    store_in(&mut transaction, store.account, &next).await?;
    transaction.commit().await?;
    tracing::info!(event = "operating.applied", account_id = %store.account);
    metrics::counter!("clacks_client_operating_applications_total").increment(1);
    Ok(OperatingSaveOutcome::Saved(next))
}
