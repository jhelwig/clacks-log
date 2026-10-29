//! Account admission and durable acknowledgment records.

use crate::{
    account::{AccountMetadata, InstalledCheckpoint},
    error::MetadataError,
    record::Position,
};
use clacks_model::ids::{AccountId, ClientActorId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Admission lifetime and last durably installed checkpoint of a client.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientMembership {
    /// Current enrollment/writer identity.
    pub actor: ClientActorId,
    /// Account position at authorization.
    pub created_at: Position,
    /// Explicit cutoff for automatic proposals, when revoked.
    pub closed_at: Option<Position>,
    /// Last checkpoint reloaded from the client's committed store.
    pub acknowledged: Option<InstalledCheckpoint>,
}

/// Membership read from the same transaction view as account metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipSnapshot {
    /// Owning account.
    pub account: AccountId,
    /// Admission version guarding concurrent changes.
    pub generation: u64,
    /// Active and not-yet-collected closed identities.
    pub clients: Vec<ClientMembership>,
}

impl MembershipSnapshot {
    /// Validate account, epoch, unique identities and acknowledgment scope.
    pub fn validate(&self, account: &AccountMetadata) -> Result<(), MetadataError> {
        account.validate()?;
        crate::error::validate_counter(self.generation, "membership generation")?;
        if self.account != account.account || self.generation != account.membership_generation {
            return Err(MetadataError::Scope {
                detail: format!(
                    "membership account/epoch differs from {} at generation {}",
                    account.account, account.membership_generation
                ),
            });
        }
        let mut ids = BTreeSet::new();
        for client in &self.clients {
            client.created_at.validate()?;
            if let Some(acknowledged) = &client.acknowledged {
                acknowledged.validate()?;
            }
            if let Some(closed) = client.closed_at {
                closed.validate()?;
            }
            if !ids.insert(client.actor) {
                return Err(MetadataError::Conflict {
                    detail: format!("duplicate membership {}", client.actor),
                });
            }
            if client
                .acknowledged
                .as_ref()
                .is_some_and(|ack| ack.account != self.account)
            {
                return Err(MetadataError::Scope {
                    detail: format!("client {} acknowledgment crosses account", client.actor),
                });
            }
        }
        Ok(())
    }
}

/// Atomic admission update guarded by the observed account head and epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MembershipUpdate {
    /// Required current synchronization head.
    pub expected_head: Position,
    /// Required current admission generation.
    pub expected_generation: u64,
    /// New membership rows.
    pub next: MembershipSnapshot,
    /// Account revision and admission epoch to save together.
    pub next_account: AccountMetadata,
}

/// Admit a new writer without claiming that it has installed any shared checkpoint.
///
/// Commit the returned head and membership together under the observed guards.
/// Existing identities cannot be reopened through this operation.
pub fn enroll(
    actor: ClientActorId,
    account: &AccountMetadata,
    membership: &MembershipSnapshot,
) -> anyhow::Result<MembershipUpdate> {
    membership.validate(account)?;
    anyhow::ensure!(
        !membership
            .clients
            .iter()
            .any(|client| client.actor == actor),
        "client actor already has an admission history"
    );
    let mut next_account = account.advance()?;
    next_account.membership_generation =
        crate::error::next_counter(membership.generation, "membership generation")?;
    let mut next = membership.clone();
    next.generation = next_account.membership_generation;
    next.clients.push(ClientMembership {
        actor,
        created_at: next_account.head,
        closed_at: None,
        acknowledged: None,
    });
    Ok(MembershipUpdate {
        expected_head: account.head,
        expected_generation: membership.generation,
        next,
        next_account,
    })
}

/// Close automatic admission without expiring other clients or deleting accepted data.
///
/// Save the returned account and membership rows in one conditional transaction.
#[tracing::instrument(
    skip_all,
    name = "sync.membership.revoke.plan",
    level = "debug",
    fields(
        account_id = %account.account,
        actor_id = %actor,
    ),
)]
pub fn revoke(
    actor: ClientActorId,
    account: &AccountMetadata,
    membership: &MembershipSnapshot,
) -> anyhow::Result<MembershipUpdate> {
    membership.validate(account)?;
    let mut next = membership.clone();
    let client = next
        .clients
        .iter_mut()
        .find(|c| c.actor == actor)
        .ok_or_else(|| MetadataError::Invalid {
            detail: format!("cannot revoke unknown actor {actor}"),
        })?;
    if client.closed_at.is_some() {
        return Ok(MembershipUpdate {
            expected_head: account.head,
            expected_generation: membership.generation,
            next,
            next_account: account.clone(),
        });
    }
    let mut next_account = account.advance()?;
    next_account.membership_generation =
        crate::error::next_counter(membership.generation, "membership generation")?;
    client.closed_at = Some(next_account.head);
    next.generation = next_account.membership_generation;
    Ok(MembershipUpdate {
        expected_head: account.head,
        expected_generation: membership.generation,
        next,
        next_account,
    })
}
