//! Saved QSO lookups for import review.

use super::ReadSnapshot;
use crate::payload::contact;
use clacks_model::{
    contact::BandFrequency,
    ids::QsoId,
    recognition::mode::Mode,
    scalar::{RecordedCallsign, UtcSecond},
};
use std::ops::RangeInclusive;

/// A saved QSO's values for import matching, including pending local edits.
pub struct ImportIdentity {
    pub id: QsoId,
    pub own_call: RecordedCallsign,
    pub start: UtcSecond,
    pub primary: BandFrequency,
    pub mode: Mode,
}

impl ReadSnapshot {
    /// Load account reference definitions and service rules from this snapshot.
    pub async fn import_context(&mut self) -> anyhow::Result<super::projection::ReadContext> {
        super::projection::context(&mut self.transaction, self.account).await
    }

    /// Find saved QSOs with the exact worked callsign and a start in the
    /// inclusive Unix-second range. Sort by start, then by QSO ID.
    pub async fn import_identities(
        &mut self,
        call: &RecordedCallsign,
        window: RangeInclusive<i64>,
    ) -> anyhow::Result<Vec<ImportIdentity>> {
        let account = self.account.to_string();
        let call = call.as_str();
        let (lower, upper) = window.into_inner();
        sqlx::query_file!("sql/import/candidates.sql", account, call, lower, upper)
            .fetch_all(&mut *self.transaction)
            .await?
            .into_iter()
            .map(|row| {
                Ok(ImportIdentity {
                    id: row.record_id.parse()?,
                    own_call: RecordedCallsign::new(&row.own_call)?,
                    start: UtcSecond::from_unix_seconds(row.start),
                    primary: contact::pair(row.frequency, row.band)?,
                    mode: contact::mode(&row.mode_kind, row.mode_label, row.mode_parent)?,
                })
            })
            .collect()
    }
}
