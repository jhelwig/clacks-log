//! Publish eligible actual saved work in reference order after complete inbound receipt.

use super::{Socket, exchange::response, send};
use clacks_client_store::{
    store::ClientStore,
    synchronization::{self, PrepareOutcome},
};
use clacks_sync::{
    client::Preparation,
    protocol::messages::{ClientMessage, ProposalOutcome, ServerMessage},
    record::{RecordId, RecordState},
};
use tokio_util::sync::CancellationToken;

/// Publish each eligible coalesced record once; unresolved work waits for a real state change.
pub(super) async fn publish(
    socket: &mut Socket,
    store: &ClientStore,
    dirty: &mut bool,
    stop: &CancellationToken,
) -> anyhow::Result<()> {
    let mut pending = synchronization::state(store).await?.pending;
    pending.retain(|row| row.review.is_none());
    pending.sort_by_key(
        |row| match (row.record.key().id, row.record.working_state()) {
            (RecordId::Qso(_), Some(RecordState::Deleted)) => 0,
            (RecordId::Tag(_) | RecordId::Equipment(_), Some(RecordState::Deleted)) => 4,
            (RecordId::Profile(_), _) => 2,
            (RecordId::Qso(_), _) => 3,
            _ => 1,
        },
    );
    let mut request = 0_u64;
    while !pending.is_empty() {
        let ready = pending.iter().position(|row| {
            row.record.working_state().is_some_and(|state| {
                clacks_sync::acceptance::required_references(row.record.key(), state)
                    .iter()
                    .all(|dependency| {
                        !pending
                            .iter()
                            .any(|other| other.record.key() == *dependency)
                    })
            })
        });
        let Some(index) = ready else { break };
        let row = pending.remove(index);
        let key = row.record.key();
        let PrepareOutcome::Core(preparation) = synchronization::prepare(store, key).await? else {
            continue;
        };
        let change = match *preparation {
            Preparation::Ready { change, .. } | Preparation::Retry { change, .. } => change,
            _ => continue,
        };
        send(
            socket,
            stop,
            ClientMessage::Propose {
                request,
                change: change.clone(),
            },
        )
        .await?;
        let ServerMessage::Proposal {
            request: actual,
            outcome,
        } = response(socket, store, dirty, stop).await?
        else {
            anyhow::bail!("Server did not return proposal outcome");
        };
        anyhow::ensure!(actual == request, "Wrong proposal response");
        match outcome {
            ProposalOutcome::Accepted(value) | ProposalOutcome::Covered(value) => {
                synchronization::receive(store, &value).await?;
                *dirty = true;
            }
            ProposalOutcome::Stale(observed) => {
                synchronization::stale(store, &change, &observed).await?;
                *dirty = true;
            }
            ProposalOutcome::Rejected(reason) => {
                synchronization::reject(store, key, &reason).await?;
            }
        }
        request = request
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Proposal sequence exhausted"))?;
        if stop.is_cancelled() {
            return Ok(());
        }
    }
    Ok(())
}
