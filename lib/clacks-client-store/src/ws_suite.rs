//! Workspace-local WS Suite receiver preferences, separate from account records.

use crate::{local_state, store::ClientStore};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};

/// Local-state key owned by this receiver.
const KEY: &str = "ws_suite.receiver";

/// Persisted receive intent and last committed connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiverSettings {
    /// Whether opening the workspace should start reception.
    pub enabled: bool,
    /// Connection retained even while reception is disabled.
    pub connection: ReceiverConnection,
}

/// One receiving mode; selecting multicast requires an explicit group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiverConnection {
    /// Receive datagrams addressed directly to this computer.
    Unicast {
        /// Local bind address, including an explicitly chosen wildcard.
        address: IpAddr,
        /// UDP port, from 1 through 65535.
        port: u16,
    },
    /// Join one multicast group on one interface.
    Multicast {
        /// IPv4 or IPv6 multicast destination.
        group: IpAddr,
        /// Shared UDP port.
        port: u16,
        /// Membership interface selection.
        interface: InterfaceSelection,
    },
}

/// Stable interface selection; OS numeric indexes are resolved at activation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterfaceSelection {
    /// Ask the OS for the appropriate membership interface.
    SystemDefault,
    /// Resolve exactly this named local address, without substituting adapters.
    Explicit {
        /// OS adapter name.
        name: String,
        /// Selected local address.
        address: IpAddr,
    },
}

impl Default for ReceiverSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            connection: ReceiverConnection::Unicast {
                address: Ipv4Addr::LOCALHOST.into(),
                port: 2237,
            },
        }
    }
}

impl ReceiverSettings {
    /// Check intrinsic address, port and scope rules; availability is checked at activation.
    /// Disabled settings retain their committed connection without requiring it to be usable.
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        match &self.connection {
            ReceiverConnection::Unicast { address, port } => {
                anyhow::ensure!(*port != 0, "Use a port from 1 to 65535.");
                anyhow::ensure!(
                    !address.is_multicast() && *address != IpAddr::V4(Ipv4Addr::BROADCAST),
                    "Enter a unicast local address."
                );
            }
            ReceiverConnection::Multicast {
                group,
                port,
                interface,
            } => {
                anyhow::ensure!(*port != 0, "Use a port from 1 to 65535.");
                anyhow::ensure!(group.is_multicast(), "Enter a multicast group address.");
                match interface {
                    InterfaceSelection::Explicit { name, address } => {
                        anyhow::ensure!(
                            !name.is_empty() && address.is_ipv4() == group.is_ipv4(),
                            "Select a matching network interface."
                        );
                    }
                    InterfaceSelection::SystemDefault => {
                        if let IpAddr::V6(group) = group {
                            let [_, scope, ..] = group.octets();
                            anyhow::ensure!(
                                scope & 0x0f > 2,
                                "Select an explicit interface for local-scope IPv6 multicast."
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Result of a preference apply, distinguishing actual mutation from unchanged settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiverSaveOutcome {
    /// Committed receiver settings.
    pub settings: ReceiverSettings,
    /// Whether this operation wrote a different preference value.
    pub changed: bool,
}

/// Load the receiver preference, defaulting without a write on an absent key.
/// Malformed stored settings return an error rather than silently replacing intent.
pub async fn load(store: &ClientStore) -> anyhow::Result<ReceiverSettings> {
    let mut tx = store.pool.begin().await?;
    let value = local_state::get_in(&mut tx, KEY).await?;
    let settings: ReceiverSettings = match value {
        Some(value) => serde_json::from_str(&value)?,
        None => ReceiverSettings::default(),
    };
    settings.validate()?;
    tx.commit().await?;
    Ok(settings)
}

/// Commit validated receiver intent in one local transaction; unchanged input does not write.
/// Disabled applies preserve the saved connection and discard pending connection edits.
#[tracing::instrument(skip_all, fields(enabled = next.enabled))]
pub async fn save(
    store: &ClientStore,
    next: &ReceiverSettings,
) -> anyhow::Result<ReceiverSaveOutcome> {
    let mut tx = store.write().await?;
    let current: ReceiverSettings = match local_state::get_in(&mut tx, KEY).await? {
        Some(value) => serde_json::from_str(&value)?,
        None => ReceiverSettings::default(),
    };
    let next = if next.enabled {
        next.clone()
    } else {
        ReceiverSettings {
            enabled: false,
            connection: current.connection.clone(),
        }
    };
    next.validate()?;
    let changed = current != next;
    if changed {
        local_state::set_in(&mut tx, KEY, &serde_json::to_string(&next)?).await?;
    }
    tx.commit().await?;
    if changed {
        tracing::info!(event = "ws_suite.settings_saved", enabled = next.enabled);
    }
    Ok(ReceiverSaveOutcome {
        settings: next,
        changed,
    })
}
