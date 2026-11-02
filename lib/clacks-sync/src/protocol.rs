//! Shared application wire values used before synchronization begins.

pub mod directory;
pub mod messages;

use clacks_model::ids::{AccountId, ClientActorId};
use serde::{Deserialize, Serialize};

/// Account and admitted actor verified by a proof-bound identity request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientIdentity {
    /// Owning Clacks account.
    pub account: AccountId,
    /// Active client admission, not the public OAuth application ID.
    pub actor: ClientActorId,
}
