//! Received interpretation, retained pending comparison evidence and local relational cleanup.

use crate::metadata::{client, lineage};
use clacks_sync::{
    account::InstalledCheckpoint,
    lineage::Lineage,
    record::{Observation, RecordKey},
};
use sqlx::SqliteConnection;

/// Incorporate received ancestry without forgetting previously learned immutable boundaries.
pub(crate) async fn merge_lineage(
    connection: &mut SqliteConnection,
    received: &Lineage,
) -> anyhow::Result<()> {
    let current = lineage::interpretation(connection, received.account()).await?;
    let existed = current.is_some();
    let mut merged = match current {
        Some(current) => {
            anyhow::ensure!(
                current.lineage.initial_timeline() == received.initial_timeline(),
                "received ancestry has a different initial timeline"
            );
            current.lineage
        }
        None => received.clone(),
    };
    let previous = merged.clone();
    for boundary in received.boundaries() {
        merged.add_boundary(boundary.clone())?;
    }
    if !existed || merged != previous {
        lineage::set_lineage(connection, &merged).await?;
    }
    Ok(())
}

/// Retain self-contained pending bases before changing received comparison interpretation.
///
/// Insufficient coverage retains the original and all its interpretation roots.
/// Other integrity/scope failures abort the transaction instead of discarding evidence.
pub(crate) async fn seal_pending(
    connection: &mut SqliteConnection,
    checkpoint: &InstalledCheckpoint,
) -> anyhow::Result<()> {
    let Some(context) = lineage::interpretation(connection, checkpoint.account).await? else {
        return Ok(());
    };
    for key in sqlx::query_file!("sql/retention/pending.sql")
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account: key.account.parse()?,
            id: crate::payload::record_id(&key.kind, &key.record_id)?,
        };
        let row = client::load(connection, key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("pending row disappeared"))?;
        let mut fields = row.into_fields();
        let Some(pending) = &mut fields.pending else {
            continue;
        };
        if pending.witness.is_some() {
            continue;
        }
        let Observation::Accepted(observed) = &fields.latest else {
            continue;
        };
        match clacks_sync::compaction::seal_basis(pending, observed, checkpoint, &context.view()) {
            Ok(witness) => pending.witness = Some(witness),
            Err(error)
                if matches!(
                    error.downcast_ref::<clacks_sync::error::MetadataError>(),
                    Some(clacks_sync::error::MetadataError::Incomplete { .. })
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        }
        let row = fields.try_into()?;
        client::store(connection, &row).await?;
    }
    Ok(())
}
