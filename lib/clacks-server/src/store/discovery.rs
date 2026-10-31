//! Short consistent accepted-state reads with fixed complete targets and immutable cursors.

use super::{
    AccountStore,
    metadata::{account, lineage},
    payload, synchronization,
};
use crate::enrollment::AuthorizedClient;
use clacks_sync::{
    account::InstalledCheckpoint,
    record::RecordKey,
    transfer::{
        RecordRequirement, WindowMode,
        discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
    },
};

pub(crate) mod support;

/// A source condition that prevents certifying the current discovery.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SourceError {
    /// Previously selected complete coverage is no longer represented by the source.
    #[error(
        "The discovery target is no longer available. The client will reconnect and negotiate again."
    )]
    TargetUnavailable,
    /// A selected incremental interval no longer has its required journal.
    #[error(
        "The discovery journal interval is no longer available. The client will reconnect and negotiate again."
    )]
    JournalUnavailable,
    /// A required exact accepted prerequisite is absent.
    #[error("The discovery source lacks a required accepted supporting version.")]
    MissingSupport,
}

/// Enumeration progress for a single live discovery; not a persistent snapshot or coverage claim.
pub(crate) struct Source {
    /// Fixed scope selected before enumeration begins.
    pub(crate) start: DiscoveryStart,
    /// Last immutable identity visited by a full traversal.
    kind: String,
    /// Last native identifier visited within that kind.
    id: String,
    /// Last installation timeline visited by a journal traversal.
    timeline: String,
    /// Last installation revision visited within that timeline.
    revision: i64,
    /// Number of pages supplied in this attempt.
    sequence: u64,
    /// Exhaustion was checked under a valid source read.
    ended: bool,
}

impl Source {
    /// Capture real account coverage and choose an available journal or a full accepted traversal.
    #[tracing::instrument(skip_all, name="server.sync.discovery.start", level="debug", fields(account_id=%client.account, actor_id=%client.actor))]
    pub(crate) async fn start(
        store: &AccountStore,
        client: &AuthorizedClient,
        after: Option<&InstalledCheckpoint>,
    ) -> anyhow::Result<Self> {
        let mut transaction = store
            .pool
            .begin_with("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await?;
        crate::oauth::resource::check_current(&mut transaction, client).await?;
        let account = account::load(&mut transaction, client.account).await?;
        let interpretation = account::interpretation(&mut transaction, client.account).await?;
        let source = synchronization::checkpoint(&account);
        let owner = client.account.to_string();
        let row = sqlx::query_file!("sql/metadata/state_select.sql", owner)
            .fetch_one(&mut *transaction)
            .await?;
        let retained = lineage::load_coverage(
            &mut transaction,
            client.account,
            row.retained_journal_coverage_id,
        )
        .await?;
        let mode = clacks_sync::transfer::discovery::select_mode(
            &source,
            after,
            Some(&retained),
            &interpretation.view(),
        )?;
        transaction.commit().await?;
        Ok(Self {
            start: DiscoveryStart { source, mode },
            kind: String::new(),
            id: String::new(),
            timeline: String::new(),
            revision: -1,
            sequence: 0,
            ended: false,
        })
    }

    /// Read a bounded page without retaining a database transaction across network delivery.
    ///
    /// A newer head may satisfy an older journal obligation, but cannot enlarge this target.
    #[tracing::instrument(skip_all, name="server.sync.discovery.page", level="debug", fields(account_id=%client.account, page=self.sequence, limit))]
    pub(crate) async fn page(
        &mut self,
        store: &AccountStore,
        client: &AuthorizedClient,
        limit: u16,
    ) -> anyhow::Result<Option<DiscoveryPage>> {
        anyhow::ensure!(
            client.account == self.start.source.account && !self.ended && limit > 0,
            "invalid discovery scope or state"
        );
        let mut transaction = store
            .pool
            .begin_with("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await?;
        crate::oauth::resource::check_current(&mut transaction, client).await?;
        let interpretation = account::interpretation(&mut transaction, client.account).await?;
        let target = clacks_sync::transfer::discovery::target_coverage(
            &self.start.source,
            &interpretation.view(),
        )?;
        let current = account::load(&mut transaction, client.account).await?;
        anyhow::ensure!(
            target.entries().all(|(timeline, revision)| current
                .installed
                .covers(clacks_sync::record::Position { timeline, revision })),
            SourceError::TargetUnavailable
        );
        let owner = client.account.to_string();
        let mut requirements = Vec::new();
        let mut accepted = Vec::new();
        let mut support = Vec::new();
        match &self.start.mode {
            WindowMode::Full => {
                for row in sqlx::query_file!(
                    "sql/discovery/full.sql",
                    owner,
                    self.kind,
                    self.id,
                    i64::from(limit)
                )
                .fetch_all(&mut *transaction)
                .await?
                {
                    let key = RecordKey {
                        account: client.account,
                        id: payload::record_id(&row.kind, &row.record_id)?,
                    };
                    if let clacks_sync::record::Observation::Accepted(value) =
                        synchronization::context::observation(&mut transaction, &current, key)
                            .await?
                    {
                        requirements.push(RecordRequirement {
                            key,
                            stamp: value.stamp.clone(),
                            deleted: matches!(
                                value.state(),
                                clacks_sync::record::RecordState::Deleted
                            ),
                        });
                        accepted.push(value);
                    }
                    for retained in sqlx::query_file!(
                        "sql/discovery/retained.sql",
                        owner,
                        row.kind,
                        row.record_id
                    )
                    .fetch_all(&mut *transaction)
                    .await?
                    {
                        let value = super::metadata::accepted::load_envelope(
                            &mut transaction,
                            key,
                            retained.envelope_id,
                        )
                        .await?;
                        if !accepted.contains(&value) && !support.contains(&value) {
                            support.push(value);
                        }
                    }
                    self.kind = row.kind;
                    self.id = row.record_id;
                }
            }
            WindowMode::Incremental { after } => {
                let row = sqlx::query_file!("sql/metadata/state_select.sql", owner)
                    .fetch_one(&mut *transaction)
                    .await?;
                let retained = lineage::load_coverage(
                    &mut transaction,
                    client.account,
                    row.retained_journal_coverage_id,
                )
                .await?;
                anyhow::ensure!(
                    retained.entries().all(|(timeline, revision)| after
                        .covers(clacks_sync::record::Position { timeline, revision })),
                    SourceError::JournalUnavailable
                );
                let mut timelines = Vec::new();
                let mut lower = Vec::new();
                let mut upper = Vec::new();
                for (timeline, revision) in target.entries() {
                    timelines.push(timeline.to_string());
                    lower.push(i64::try_from(
                        after
                            .entries()
                            .find(|(id, _)| *id == timeline)
                            .map_or(0, |(_, rev)| rev),
                    )?);
                    upper.push(i64::try_from(revision)?);
                }
                for row in sqlx::query_file!(
                    "sql/discovery/journal.sql",
                    owner,
                    &timelines,
                    &lower,
                    &upper,
                    self.timeline,
                    self.revision,
                    i64::from(limit)
                )
                .fetch_all(&mut *transaction)
                .await?
                {
                    let key = RecordKey {
                        account: client.account,
                        id: payload::record_id(&row.kind, &row.record_id)?,
                    };
                    let stamp =
                        super::metadata::clock::load_stamp(&mut transaction, key, row.stamp_id)
                            .await?;
                    let value = super::metadata::accepted::load_envelope(
                        &mut transaction,
                        key,
                        row.envelope_id,
                    )
                    .await?;
                    // Deletion status belongs to the exact obligation, not necessarily its successor.
                    let exact = synchronization::context::exact(
                        &mut transaction,
                        &clacks_sync::service::VersionReference {
                            key,
                            stamp: stamp.clone(),
                            deleted: true,
                        },
                    )
                    .await?;
                    requirements.push(RecordRequirement {
                        key,
                        stamp,
                        deleted: exact.is_some(),
                    });
                    accepted.push(value);
                    self.timeline = row.timeline_id;
                    self.revision = row.revision;
                }
            }
        }
        if accepted.is_empty() && support.is_empty() && self.sequence > 0 {
            transaction.commit().await?;
            self.ended = true;
            return Ok(None);
        }
        let units = support::load(
            &mut transaction,
            &current,
            &mut accepted,
            &mut support,
            true,
        )
        .await?;
        let page = DiscoveryPage {
            sequence: self.sequence,
            requirements,
            accepted,
            support,
            baselines: interpretation.baselines,
            boundaries: interpretation.lineage.boundaries().to_vec(),
            units,
        };
        transaction.commit().await?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("discovery sequence exhausted"))?;
        Ok(Some(page))
    }

    /// Certify only an enumeration whose final empty range was checked successfully.
    pub(crate) fn end(&self) -> anyhow::Result<DiscoveryEnd> {
        anyhow::ensure!(self.ended, "discovery has not exhausted its source");
        Ok(DiscoveryEnd {
            source: self.start.source.clone(),
            pages: self.sequence,
        })
    }
}
