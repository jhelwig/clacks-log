//! Account-wide pending and conflict review using native identity-qualified paging.

use super::ReadSnapshot;
use clacks_model::ids::ClientActorId;
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

/// Account-wide pending selection with optional section and native-category filters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewQuery {
    /// Limit to one actionable or ordinary pending section.
    pub section: Option<ReviewSection>,
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
            section: None,
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
    /// Former writer whose unsubmitted work requires explicit admission.
    pub authorization_hold: Option<ClientActorId>,
}

/// Presentation category derived from persisted review and admission state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewSection {
    /// Explicit review, admission hold or retained recovery evidence.
    Conflicts,
    /// Ordinary local work with no current resolution requirement.
    PendingSync,
}

/// Full filtered record counts, independent of page size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewCounts {
    /// Records requiring review or waiting for comparison evidence.
    pub conflicts: u64,
    /// Ordinary unpublished records.
    pub pending_sync: u64,
}

/// Bounded native rows and their continuation within one read snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewPage {
    /// Complete roles for the displayed page.
    pub rows: Vec<ReviewRow>,
    /// Counts across all records matching the type filter.
    pub counts: ReviewCounts,
    /// Last returned identity when another matching row exists.
    pub next: Option<RecordId>,
}

impl ReadSnapshot {
    /// Read pending and unresolved records account-wide, including deleted and untagged values.
    #[tracing::instrument(skip_all, name = "client_store.review.read", level = "debug", fields(account_id = %self.account, limit = query.limit))]
    pub async fn review(&mut self, query: &ReviewQuery) -> anyhow::Result<Vec<ReviewRow>> {
        Ok(self.review_page(query).await?.rows)
    }

    /// Read one bounded section and full counts using the same durable snapshot.
    ///
    /// Use the returned continuation only with the same section/type selection.
    #[tracing::instrument(skip_all, name = "client_store.review.page", level = "debug", fields(account_id = %self.account, section = ?query.section, limit = query.limit))]
    pub async fn review_page(&mut self, query: &ReviewQuery) -> anyhow::Result<ReviewPage> {
        anyhow::ensure!(query.limit > 0, "review page limit must be positive");
        let account = self.account.to_string();
        let kind = query.kind.map(RecordKind::discriminator);
        let section = query
            .section
            .map(|value| i64::from(value == ReviewSection::Conflicts));
        let after_kind = query.after.map(crate::payload::kind);
        let after_id = query.after.map(|id| id.to_string());
        let limit = i64::from(query.limit) + 1;
        let counts = sqlx::query_file!("sql/read/review/counts.sql", account, kind)
            .fetch_one(&mut *self.transaction)
            .await?;
        let selected = sqlx::query_file!(
            "sql/read/review/page.sql",
            account,
            kind,
            section,
            after_kind,
            after_id,
            limit
        )
        .fetch_all(&mut *self.transaction)
        .await?;
        let has_more = selected.len() > usize::try_from(query.limit)?;
        let mut rows = Vec::new();
        for row in selected.into_iter().take(usize::try_from(query.limit)?) {
            let key = RecordKey {
                account: self.account,
                id: crate::payload::record_id(&row.kind, &row.record_id)?,
            };
            rows.push(
                self.review_record(key)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("review selected a missing record"))?,
            );
        }
        let next = has_more
            .then(|| rows.last().map(|row| row.record.key().id))
            .flatten();
        Ok(ReviewPage {
            rows,
            next,
            counts: ReviewCounts {
                conflicts: u64::try_from(counts.conflicts)?,
                pending_sync: u64::try_from(counts.pending_sync)?,
            },
        })
    }

    /// Read the exact roles and comparison for one identity, even after it settles.
    #[tracing::instrument(skip_all, name = "client_store.review.record", level = "debug", fields(account_id = %self.account, record_id = %key.id))]
    pub async fn review_record(&mut self, key: RecordKey) -> anyhow::Result<Option<ReviewRow>> {
        anyhow::ensure!(
            key.account == self.account,
            "review belongs to another account"
        );
        let Some(record) = crate::metadata::client::load(&mut self.transaction, key).await? else {
            return Ok(None);
        };
        Ok(Some(ReviewRow {
            record,
            review: crate::metadata::review::load(&mut self.transaction, key).await?,
            authorization_hold: crate::enrollment::holds::actor_in(&mut self.transaction, key)
                .await?,
        }))
    }
}
