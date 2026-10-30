//! SQLite storage for copied station, operator, power and equipment values.

use crate::payload::contact;
use clacks_model::{
    ids::AccountId,
    profile::CurrentOperatingValues,
    scalar::{Milliwatts, RecordedCallsign},
};
use sqlx::SqliteConnection;

/// Store a validated copy of the operating values in the caller's transaction.
pub(super) async fn insert_value(
    connection: &mut SqliteConnection,
    account: AccountId,
    value: &CurrentOperatingValues,
) -> anyhow::Result<i64> {
    let station = contact::insert_station(connection, account, &value.station).await?;
    let equipment = match &value.equipment {
        Some(value) => Some(contact::insert_equipment(connection, account, value).await?),
        None => None,
    };
    let call = value
        .operator
        .as_ref()
        .and_then(|value| value.callsign().as_ref())
        .map(RecordedCallsign::as_str);
    let name = value
        .operator
        .as_ref()
        .and_then(|value| value.recorded_name().as_deref());
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

/// Load and validate copied operating values without looking up saved profiles.
pub(super) async fn load_value(
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
        (call, name) => Some(clacks_model::contact::OperatorInformationFields {
            callsign: call.as_deref().map(RecordedCallsign::new).transpose()?,
            recorded_name: name,
        }),
    };
    let value = clacks_model::profile::CurrentOperatingValues {
        station,
        equipment,
        operator: operator.map(TryInto::try_into).transpose()?,
        transmit_power: row
            .power
            .map(|value| -> anyhow::Result<_> { Ok(Milliwatts::new(u64::try_from(value)?)?) })
            .transpose()?,
    };

    Ok(value)
}
