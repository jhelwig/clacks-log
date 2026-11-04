//! Standard OS interface enumeration for receiver configuration.
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use std::net::IpAddr;

/// Transient local address and membership index; never persist the numeric index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InterfaceChoice {
    /// OS adapter name.
    pub name: String,
    /// Local address on that adapter.
    pub address: IpAddr,
    /// Current OS interface index, used for IPv6 scope/membership.
    pub index: u32,
}

/// Enumerate named addresses; callers run this blocking OS operation off the UI thread.
pub(crate) fn enumerate() -> anyhow::Result<Vec<InterfaceChoice>> {
    let mut choices = Vec::new();
    for interface in NetworkInterface::show()? {
        for address in interface.addr {
            choices.push(InterfaceChoice {
                name: interface.name.clone(),
                address: address.ip(),
                index: interface.index,
            });
        }
    }
    choices.sort_by(|a, b| a.name.cmp(&b.name).then(a.address.cmp(&b.address)));
    choices.dedup();
    Ok(choices)
}
