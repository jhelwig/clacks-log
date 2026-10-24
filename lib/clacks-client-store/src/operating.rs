//! Copied client-local operating defaults and explicit profile loading.

use crate::rows::operating::OperatingStateRow;
use crate::{metadata::client, payload, payload::contact, store::ClientStore};
use clacks_model::{
    contact::OperatorInformation,
    ids::AccountId,
    profile::{CurrentOperatingValues, LoadedProfileBaseline},
    scalar::{Milliwatts, RecordedCallsign},
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use sqlx::SqliteConnection;

/// Local entry defaults and the copies captured by the last explicit profile load.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperatingState {
    /// Current defaults, absent before an operating location has been selected.
    pub current: Option<CurrentOperatingValues>,
    /// Complete loaded copies used to indicate subsequent changes.
    pub loaded: Option<LoadedProfileBaseline>,
}

/// Result of checking the caller's complete operating-state basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperatingSaveOutcome {
    /// Committed operating state.
    Saved(OperatingState),
    /// Current state to reload before applying the caller's intent again.
    Stale(OperatingState),
}

/// Store a validated owned operating snapshot in the caller's transaction.
async fn insert_value(
    connection: &mut SqliteConnection,
    account: AccountId,
    value: &CurrentOperatingValues,
) -> anyhow::Result<i64> {
    value.validate()?;
    let station = contact::insert_station(connection, account, &value.station).await?;
    let equipment = match &value.equipment {
        Some(value) => Some(contact::insert_equipment(connection, account, value).await?),
        None => None,
    };
    let call = value
        .operator
        .as_ref()
        .and_then(|value| value.callsign.as_ref())
        .map(RecordedCallsign::as_str);
    let name = value
        .operator
        .as_ref()
        .and_then(|value| value.recorded_name.as_deref());
    let power = value
        .transmit_power
        .map(|value| i64::try_from(value.get()))
        .transpose()?;
    let account = account.to_string();
    Ok(sqlx::query_file!(
        "sql/operating/value_insert.sql",
        account,
        station,
        call,
        name,
        power,
        equipment
    )
    .fetch_one(connection)
    .await?
    .operating_id)
}

/// Restore and validate an owned operating snapshot without shared-record lookups.
async fn load_value(
    connection: &mut SqliteConnection,
    account: AccountId,
    id: i64,
) -> anyhow::Result<CurrentOperatingValues> {
    let text = account.to_string();
    let row = sqlx::query_file!("sql/operating/value_select.sql", id, text)
        .fetch_one(&mut *connection)
        .await?;
    let station = contact::load_station(connection, account, row.station_id).await?;
    let equipment = match row.equipment_id {
        Some(id) => Some(contact::load_equipment(connection, account, id).await?),
        None => None,
    };
    let operator = match (row.operator_call, row.operator_name) {
        (None, None) => None,
        (call, name) => Some(OperatorInformation {
            callsign: call.as_deref().map(RecordedCallsign::new).transpose()?,
            recorded_name: name,
        }),
    };
    let value = CurrentOperatingValues {
        station,
        equipment,
        operator,
        transmit_power: row
            .power
            .map(|value| -> anyhow::Result<_> { Ok(Milliwatts::new(u64::try_from(value)?)?) })
            .transpose()?,
    };
    value.validate()?;
    Ok(value)
}

/// Read defaults and loaded copies using the caller's existing snapshot.
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
        Some(id) => Some(load_value(connection, account, id).await?),
        None => None,
    };
    let loaded = match row {
        OperatingStateRow {
            loaded_profile: None,
            profile_record: None,
            loaded_values: None,
            ..
        } => None,
        OperatingStateRow {
            loaded_profile: Some(payload),
            profile_record: Some(id),
            loaded_values: Some(values),
            ..
        } => {
            let key = RecordKey {
                account,
                id: RecordId::Profile(id.parse()?),
            };
            let RecordState::Live(RecordValue::Profile(profile)) =
                payload::load(connection, key, payload).await?
            else {
                anyhow::bail!("loaded profile copy is not a live profile")
            };
            Some(LoadedProfileBaseline {
                profile: *profile,
                operating_values: load_value(connection, account, values).await?,
            })
        }
        _ => anyhow::bail!("incomplete loaded profile baseline"),
    };
    Ok(OperatingState { current, loaded })
}

/// Persist complete local copies without modifying any shared profile or configuration.
async fn store_in(
    connection: &mut SqliteConnection,
    account: AccountId,
    state: &OperatingState,
) -> anyhow::Result<()> {
    let current = match &state.current {
        Some(value) => Some(insert_value(connection, account, value).await?),
        None => None,
    };
    let row = match &state.loaded {
        None => OperatingStateRow {
            current_values: current,
            loaded_profile: None,
            profile_record: None,
            loaded_values: None,
        },
        Some(loaded) => {
            let key = RecordKey {
                account,
                id: RecordId::Profile(loaded.profile.id),
            };
            let payload = payload::insert(
                connection,
                key,
                &RecordState::Live(RecordValue::Profile(Box::new(loaded.profile.clone()))),
            )
            .await?;
            OperatingStateRow {
                current_values: current,
                loaded_profile: Some(payload),
                profile_record: Some(loaded.profile.id.to_string()),
                loaded_values: Some(
                    insert_value(connection, account, &loaded.operating_values).await?,
                ),
            }
        }
    };
    let account = account.to_string();
    sqlx::query_file!(
        "sql/operating/state_store.sql",
        account,
        row.current_values,
        row.loaded_profile,
        row.profile_record,
        row.loaded_values
    )
    .execute(connection)
    .await?;
    Ok(())
}

/// Read the current defaults and loaded baseline from one consistent local snapshot.
pub async fn load(store: &ClientStore) -> anyhow::Result<OperatingState> {
    let mut transaction = store.pool.begin().await?;
    let state = load_in(&mut transaction, store.account).await?;
    transaction.commit().await?;
    Ok(state)
}

/// Save local defaults only if their complete read basis still matches.
#[tracing::instrument(skip_all, name = "client_store.operating.save", fields(account_id = %store.account))]
pub async fn save(
    store: &ClientStore,
    expected: &OperatingState,
    next: &OperatingState,
) -> anyhow::Result<OperatingSaveOutcome> {
    let mut transaction = store.write().await?;
    let current = load_in(&mut transaction, store.account).await?;
    if &current != expected {
        return Ok(OperatingSaveOutcome::Stale(current));
    }
    if &current != next {
        store_in(&mut transaction, store.account, next).await?;
    }
    transaction.commit().await?;
    Ok(OperatingSaveOutcome::Saved(next.clone()))
}

/// Explicitly load a current saved profile, copying its complete station and equipment defaults.
///
/// Both the operating state and supplied shared-record snapshots are checked in
/// the write transaction. Operator and power remain unchanged. A missing equipment
/// default preserves current equipment; loaded copies do not track later shared edits.
#[tracing::instrument(skip_all, name = "client_store.profile.load", fields(account_id = %store.account, profile_id = %profile.key.id))]
pub async fn load_profile(
    store: &ClientStore,
    expected: &OperatingState,
    profile: &ClientRecord,
    equipment: Option<&ClientRecord>,
) -> anyhow::Result<OperatingSaveOutcome> {
    anyhow::ensure!(
        profile.key.account == store.account,
        "profile belongs to another account"
    );
    let mut transaction = store.write().await?;
    let current = load_in(&mut transaction, store.account).await?;
    if &current != expected
        || client::load(&mut transaction, profile.key).await?.as_ref() != Some(profile)
    {
        return Ok(OperatingSaveOutcome::Stale(current));
    }
    let Some(RecordState::Live(RecordValue::Profile(profile))) = profile.working_state() else {
        anyhow::bail!("profile has no live working value")
    };
    let selected_equipment = if let Some(id) = profile.default_equipment {
        let row = equipment.ok_or(clacks_model::contact::RecordError::MissingEquipment { id })?;
        anyhow::ensure!(
            row.key
                == RecordKey {
                    account: store.account,
                    id: RecordId::Equipment(id)
                },
            "equipment snapshot does not match profile default"
        );
        if client::load(&mut transaction, row.key).await?.as_ref() != Some(row) {
            return Ok(OperatingSaveOutcome::Stale(current));
        }
        let Some(RecordState::Live(RecordValue::Equipment(value))) = row.working_state() else {
            anyhow::bail!("equipment has no live working value")
        };
        Some(value.as_ref())
    } else {
        None
    };
    let initial = CurrentOperatingValues {
        station: profile.station.clone(),
        operator: None,
        transmit_power: None,
        equipment: None,
    };
    let values = clacks_model::profile::load_profile(
        current.current.as_ref().unwrap_or(&initial),
        profile,
        selected_equipment,
    )?;
    let state = OperatingState {
        current: Some(values.clone()),
        loaded: Some(LoadedProfileBaseline {
            profile: (**profile).clone(),
            operating_values: values,
        }),
    };
    store_in(&mut transaction, store.account, &state).await?;
    transaction.commit().await?;
    tracing::info!(event = "profile.loaded", account_id = %store.account, profile_id = %profile.id);
    Ok(OperatingSaveOutcome::Saved(state))
}
