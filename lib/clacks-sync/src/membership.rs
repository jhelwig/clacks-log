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
