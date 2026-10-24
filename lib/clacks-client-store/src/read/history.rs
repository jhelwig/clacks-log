//! One local working QSO per identity with bound filters and stable keyset paging.

use super::ReadSnapshot;
use clacks_model::{
    contact::Qso,
    ids::{QsoId, TagId},
    scalar::UtcSecond,
};
use clacks_sync::{
    client::ReviewInput,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use std::collections::BTreeSet;

/// Bound history selection; empty tags select the entire account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryQuery {
    /// Any selected membership qualifies; duplicate tags cannot duplicate a QSO.
    pub tags: BTreeSet<TagId>,
    /// Literal contacted-call prefix, with surrounding whitespace ignored.
    pub contacted_call_prefix: Option<String>,
    /// Inclusive contact start lower bound.
    pub start: Option<UtcSecond>,
    /// Inclusive contact start upper bound.
    pub end: Option<UtcSecond>,
    /// Exact recorded primary band label, including unfamiliar labels.
    pub band: Option<String>,
    /// Exact recorded effective mode label, including unfamiliar labels.
    pub mode: Option<String>,
    /// Complete recorded own callsign, normalized like entry callsigns.
    pub own_call: Option<String>,
    /// Exclusive prior `(start, identity)` in ascending order within this snapshot.
    pub after: Option<(UtcSecond, QsoId)>,
    /// Positive maximum returned contacts.
    pub limit: u32,
}

impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            tags: BTreeSet::new(),
            contacted_call_prefix: None,
            start: None,
            end: None,
            band: None,
            mode: None,
            own_call: None,
            after: None,
            limit: 100,
        }
    }
}

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

impl ReadSnapshot {
    /// Read a stable page of working contacts without duplicating memberships or exposing deletions.
    #[tracing::instrument(skip_all, name = "client_store.history.read", level = "debug", fields(account_id = %self.account, limit = query.limit))]
    pub async fn history(&mut self, query: &HistoryQuery) -> anyhow::Result<Vec<HistoryRow>> {
        anyhow::ensure!(query.limit > 0, "history page limit must be positive");
        anyhow::ensure!(
            query
                .start
                .zip(query.end)
                .is_none_or(|(start, end)| start <= end),
            "history time range is reversed"
        );
        let tags = query
            .tags
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let account = self.account.to_string();
        let prefix = query.contacted_call_prefix.as_ref().map(|value| {
            format!(
                "{}%",
                value
                    .trim()
                    .to_uppercase()
                    .replace('!', "!!")
                    .replace('%', "!%")
                    .replace('_', "!_")
            )
        });
        let start = query.start.map(UtcSecond::unix_seconds);
        let end = query.end.map(UtcSecond::unix_seconds);
        let own_call = query
            .own_call
            .as_ref()
            .map(|value| value.trim().to_uppercase());
        let after = query.after.map(|(time, _)| time.unix_seconds());
        let after_id = query.after.map(|(_, id)| id.to_string());
        let limit = i64::from(query.limit);
        let rows = sqlx::query_file!(
            "sql/read/history.sql",
            tags,
            account,
            prefix,
            start,
            end,
            query.band,
            query.mode,
            own_call,
            after,
            after_id,
            limit
        )
        .fetch_all(&mut *self.transaction)
        .await?;
        let mut values = vec![];
        for row in rows {
            let key = RecordKey {
                account: self.account,
                id: RecordId::Qso(row.record_id.parse()?),
            };
            let RecordState::Live(RecordValue::Qso(qso)) =
                crate::payload::load(&mut self.transaction, key, row.payload_id).await?
            else {
                anyhow::bail!("history selected a non-live QSO")
            };
            values.push(HistoryRow {
                qso: *qso,
                pending: row.pending,
                review: crate::metadata::review::load(&mut self.transaction, key).await?,
            });
        }
        Ok(values)
    }
}
