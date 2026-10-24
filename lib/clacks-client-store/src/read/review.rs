//! Account-wide pending and conflict review using native identity-qualified paging.

use super::ReadSnapshot;
use clacks_model::ids::TagId;
use clacks_sync::{
    client::{ClientRecord, ReviewInput},
    record::{RecordId, RecordKey},
};

/// Persisted native record category for a review filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    /// Recorded contact.
    Qso,
    /// Logbook membership tag.
    Tag,
    /// Saved station profile.
    Profile,
    /// Saved equipment configuration.
    Equipment,
    /// Participation/reference definition.
    Group,
    /// Export context definition.
    OutputProfile,
    /// External service namespace.
    Destination,
    /// Imported service status assertion.
    Imported,
    /// Frozen upload attempt.
    Attempt,
    /// Upload outcome.
    Result,
    /// Downloaded service observation.
    Observation,
    /// Exact remote/local association.
    Association,
    /// Reviewed observation disposition.
    Disposition,
    /// Retained deletion suppression.
    Suppression,
    /// Service download control.
    Download,
    /// Acquisition lifecycle.
    Acquisition,
}

impl RecordKind {
    /// Select the fixed schema discriminator for this typed filter.
    fn discriminator(self) -> &'static str {
        match self {
            Self::Qso => "qso",
            Self::Tag => "tag",
            Self::Profile => "profile",
            Self::Equipment => "equipment",
            Self::Group => "group",
            Self::OutputProfile => "output_profile",
            Self::Destination => "destination",
            Self::Imported => "imported",
            Self::Attempt => "attempt",
            Self::Result => "result",
            Self::Observation => "observation",
            Self::Association => "association",
            Self::Disposition => "disposition",
            Self::Suppression => "suppression",
            Self::Download => "download",
            Self::Acquisition => "acquisition",
        }
    }
}

/// Account-wide pending selection with optional membership and native-category filters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewQuery {
    /// Match working membership; for pending deletion use retained original membership when available.
    pub tag: Option<TagId>,
    /// Restrict to one native category.
    pub kind: Option<RecordKind>,
    /// Exclusive prior identity in schema-kind/name order; identities are kind-qualified.
    pub after: Option<RecordId>,
    /// Positive maximum returned records.
    pub limit: u32,
}

impl Default for ReviewQuery {
    fn default() -> Self {
        Self {
            tag: None,
            kind: None,
            after: None,
            limit: 100,
        }
    }
}

/// Complete pending roles and any native resolution request from the same snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRow {
    /// Actual saved record, including pending deletions and retained alternatives.
    pub record: ClientRecord,
    /// Persisted core-produced review; ordinary unpublished work may have none.
    pub review: Option<ReviewInput>,
}

impl ReadSnapshot {
    /// Read pending and unresolved records account-wide, including deleted and untagged values.
    #[tracing::instrument(skip_all, name = "client_store.review.read", level = "debug", fields(account_id = %self.account, limit = query.limit))]
    pub async fn review(&mut self, query: &ReviewQuery) -> anyhow::Result<Vec<ReviewRow>> {
        anyhow::ensure!(query.limit > 0, "review page limit must be positive");
        let account = self.account.to_string();
        let kind = query.kind.map(RecordKind::discriminator);
        let tag = query.tag.map(|id| id.to_string());
        let after_kind = query.after.map(crate::payload::kind);
        let after_id = query.after.map(|id| id.to_string());
        let limit = i64::from(query.limit);
        let rows = sqlx::query_file!(
            "sql/read/pending.sql",
            account,
            kind,
            tag,
            after_kind,
            after_id,
            limit
        )
        .fetch_all(&mut *self.transaction)
        .await?;
        let mut values = vec![];
        for row in rows {
            let key = RecordKey {
                account: self.account,
                id: crate::payload::record_id(&row.kind, &row.record_id)?,
            };
            let record = crate::metadata::client::load(&mut self.transaction, key)
                .await?
                .ok_or_else(|| anyhow::anyhow!("review selected a missing record"))?;
            values.push(ReviewRow {
                record,
                review: crate::metadata::review::load(&mut self.transaction, key).await?,
            });
        }
        Ok(values)
    }
}
