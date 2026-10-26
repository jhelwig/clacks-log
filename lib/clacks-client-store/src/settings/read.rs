//! Read saved settings, logbook views and active operating values.

use super::SettingsSnapshot;
use crate::{metadata::client, operating, payload};
use clacks_model::ids::AccountId;
use clacks_sync::record::RecordKey;
use sqlx::SqliteConnection;

/// Read every settings family without conflating an unavailable working value with deletion.
pub(crate) async fn load_in(
    connection: &mut SqliteConnection,
    account: AccountId,
) -> anyhow::Result<SettingsSnapshot> {
    let mut view = SettingsSnapshot {
        logbooks: crate::logbooks::read::load_in(connection, account).await?,
        operating: operating::load_in(connection, account).await?,
        profiles: vec![],
        equipment: vec![],
        groups: vec![],
    };
    let text = account.to_string();
    for row in sqlx::query_file!("sql/settings/records.sql", text)
        .fetch_all(&mut *connection)
        .await?
    {
        let id = payload::record_id(&row.kind, &row.record_id)?;
        let record = client::load(connection, RecordKey { account, id })
            .await?
            .ok_or_else(|| anyhow::anyhow!("settings record disappeared within its snapshot"))?;
        match id {
            clacks_sync::record::RecordId::Profile(_) => view.profiles.push(record),
            clacks_sync::record::RecordId::Equipment(_) => view.equipment.push(record),
            clacks_sync::record::RecordId::Group(_) => view.groups.push(record),
            _ => anyhow::bail!("unexpected settings record kind"),
        }
    }
    Ok(view)
}
