//! Implemented native synchronization contracts.

use strum::IntoEnumIterator;

/// A synchronization representation and interpretation implemented by Clacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::EnumIter)]
pub enum SyncContract {
    /// Current native messages, synchronized values and recovery rules.
    V1,
}

impl SyncContract {
    /// WebSocket subprotocol identifier of this implementation.
    pub fn id(self) -> &'static str {
        match self {
            Self::V1 => "clacks-sync-v1",
        }
    }

    /// Resolve an exact public identifier to its implemented contract.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::iter().find(|contract| contract.id() == id)
    }
}
