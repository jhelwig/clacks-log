//! Accepted-state return from the real client store, independent of working contact projections.

mod support;

use crate::{
    metadata::{client, lineage},
    store::ClientStore,
};
use clacks_sync::transfer::{
    RecordRequirement, WindowMode,
    discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
};

/// Accepted roles supplied by a source, independently of any prefix claim.
enum Inventory {
    /// Current/retained represented state backing an actual completed target.
    Completed,
    /// Individually received represented and staged facts before a first completed target.
    Known {
        /// Genuine remote complete prefixes used to omit already represented facts.
        covered: clacks_sync::lineage::Coverage,
    },
}

/// A live full traversal with immutable native-identity paging and optional certified coverage.
pub struct Source {
    /// Accepted storage roles included by this producer.
    inventory: Inventory,
    /// Genuine completed target, absent when only individually received facts are available.
    pub start: Option<DiscoveryStart>,
    /// Native kind last visited, used only for stable enumeration.
    kind: String,
    /// Native ID last visited within that kind.
    id: String,
    /// Next ordered page within this attempt.
    sequence: u64,
    /// Final empty range was read successfully.
    ended: bool,
}

impl Source {
    /// Return accepted knowledge; lack of a completed checkpoint permits facts, never prefix claims.
    #[tracing::instrument(skip_all, name="client_store.discovery.start", level="debug", fields(account_id=%store.account))]
    pub async fn start(store: &ClientStore) -> anyhow::Result<Self> {
        let mut transaction = store.pool.begin().await?;
        let source = lineage::current_checkpoint(&mut transaction, store.account).await?;
        transaction.commit().await?;
        let inventory = if source.is_some() {
            Inventory::Completed
        } else {
            Inventory::Known {
                covered: Default::default(),
            }
        };
        Ok(Self {
            inventory,
            start: source.map(|source| DiscoveryStart {
                source,
                mode: WindowMode::Full,
            }),
            kind: String::new(),
            id: String::new(),
            sequence: 0,
            ended: false,
        })
    }

    /// Offer individual accepted knowledge not covered by the peer, plus incomplete transport input.
    ///
    /// A newer received record is useful recovery evidence even when this workspace's
    /// last complete checkpoint is older. This source never certifies those individual facts
    /// as a complete prefix and never reads unpublished working values.
    #[tracing::instrument(skip_all, name="client_store.discovery.repair", level="debug", fields(account_id=%store.account))]
    pub async fn repair(
        store: &ClientStore,
        covered: &clacks_sync::lineage::Coverage,
    ) -> anyhow::Result<Self> {
        covered.validate()?;
        let mut source = Self::start(store).await?;
        source.inventory = Inventory::Known {
            covered: covered.clone(),
        };
        source.start = None;
        Ok(source)
    }

    /// Read accepted observations and retained alternatives without transmitting unsaved/pending values.
    #[tracing::instrument(skip_all, name="client_store.discovery.page", level="debug", fields(account_id=%store.account))]
    pub async fn page(
        &mut self,
        store: &ClientStore,
        limit: u16,
    ) -> anyhow::Result<Option<DiscoveryPage>> {
        anyhow::ensure!(!self.ended && limit > 0, "invalid source read");
        let mut transaction = store.pool.begin().await?;
        let evidence = crate::synchronization::evidence(&mut transaction, store.account).await?;
        if let Some(start) = &self.start {
            let target =
                clacks_sync::transfer::discovery::target_coverage(&start.source, &evidence.view())?;
            let current = lineage::current_checkpoint(&mut transaction, store.account)
                .await?
                .ok_or_else(|| anyhow::anyhow!("completed source disappeared"))?;
            anyhow::ensure!(
                target.entries().all(|(timeline, revision)| current
                    .coverage
                    .covers(clacks_sync::record::Position { timeline, revision })),
                "source coverage no longer available"
            );
        }
        let covered = match &self.inventory {
            Inventory::Completed => "[]".to_owned(),
            Inventory::Known { covered } => serde_json::to_string(covered)?,
        };
        let mut accepted = Vec::new();
        let mut support = Vec::new();
        let mut requirements = Vec::new();
        for row in sqlx::query_file!(
            "sql/discovery/full.sql",
            store.account.to_string(),
            !matches!(self.inventory, Inventory::Completed),
            self.kind,
            self.id,
            i64::from(limit),
            covered
        )
        .fetch_all(&mut *transaction)
        .await?
        {
            let key = clacks_sync::record::RecordKey {
                account: store.account,
                id: crate::payload::record_id(&row.kind, &row.record_id)?,
            };
            if let Some(local) = client::load(&mut transaction, key).await? {
                let fields = local.into_fields();
                if let clacks_sync::record::Observation::Accepted(value) = fields.latest {
                    requirements.push(RecordRequirement {
                        key,
                        stamp: value.stamp.clone(),
                        deleted: matches!(value.state(), clacks_sync::record::RecordState::Deleted),
                    });
                    accepted.push(value);
                }
                support.extend(fields.retained);
            }
            if !matches!(self.inventory, Inventory::Completed) {
                for staged in sqlx::query_file!(
                    "sql/discovery/staged.sql",
                    store.account.to_string(),
                    row.kind,
                    row.record_id
                )
                .fetch_all(&mut *transaction)
                .await?
                {
                    let value = client::observation::load_envelope(
                        &mut transaction,
                        key,
                        staged.envelope_id,
                    )
                    .await?;
                    if !accepted.contains(&value) && !support.contains(&value) {
                        support.push(value);
                    }
                }
            }
            self.kind = row.kind;
            self.id = row.record_id;
        }
        if accepted.is_empty()
            && support.is_empty()
            && (self.sequence > 0 || !matches!(self.inventory, Inventory::Completed))
        {
            transaction.commit().await?;
            self.ended = true;
            return Ok(None);
        }
        let units = support::complete(
            &mut transaction,
            store.account,
            &mut accepted,
            &mut support,
            matches!(self.inventory, Inventory::Completed),
        )
        .await?;
        let page = DiscoveryPage {
            sequence: self.sequence,
            requirements,
            accepted,
            support,
            baselines: evidence.baselines,
            boundaries: evidence.lineage.boundaries().to_vec(),
            units,
        };
        transaction.commit().await?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("source sequence exhausted"))?;
        tracing::debug!(event="sync.discovery.page", account_id=%store.account, sequence=page.sequence, accepted=page.accepted.len(), support=page.support.len(), requirements=page.requirements.len(), units=page.units.len());
        Ok(Some(page))
    }

    /// Certify complete traversal only when this source has genuine previously completed coverage.
    pub fn end(&self) -> anyhow::Result<Option<DiscoveryEnd>> {
        anyhow::ensure!(self.ended, "source enumeration has not ended");
        Ok(self.start.as_ref().map(|start| DiscoveryEnd {
            source: start.source.clone(),
            pages: self.sequence,
        }))
    }
}
