//! Saved working contacts and their pending/conflict context.

use clacks_model::contact::Qso;
use clacks_sync::{
    client::ReviewInput,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};

/// Saved local working values with local publication/review indicators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRow {
    /// Current working contact; pending deletions are omitted from history.
    pub qso: Qso,
    /// Whether unpublished local work exists.
    pub pending: bool,
    /// Actual persisted native resolution context when one has been produced.
    pub review: Option<ReviewInput>,
}

/// Load working payloads with their actual pending/conflict evidence.
pub(super) async fn working_contacts(
    connection: &mut sqlx::SqliteConnection,
    account: clacks_model::ids::AccountId,
    rows: Vec<crate::rows::contact::WorkingContactRow>,
) -> anyhow::Result<Vec<HistoryRow>> {
    let mut values = vec![];
    for row in rows {
        let key = RecordKey {
            account,
            id: RecordId::Qso(row.record_id.parse()?),
        };
        let RecordState::Live(RecordValue::Qso(qso)) =
            crate::payload::load(connection, key, row.payload_id).await?
        else {
            anyhow::bail!("history selected a non-live QSO")
        };
        values.push(HistoryRow {
            qso: *qso,
            pending: row.pending,
            review: crate::metadata::review::load(connection, key).await?,
        });
    }
    Ok(values)
}
