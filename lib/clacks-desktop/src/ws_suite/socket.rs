//! Mode-specific pre-bind options and multicast membership using socket2.
use super::interfaces::InterfaceChoice;
use clacks_client_store::ws_suite::{InterfaceSelection, ReceiverConnection, ReceiverSettings};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6};

/// Resolve an explicit name/address without guessing a replacement adapter.
fn selected<'a>(
    selection: &InterfaceSelection,
    interfaces: &'a [InterfaceChoice],
) -> anyhow::Result<Option<&'a InterfaceChoice>> {
    let InterfaceSelection::Explicit { name, address } = selection else {
        return Ok(None);
    };
    let mut found = interfaces
        .iter()
        .filter(|entry| &entry.name == name && &entry.address == address);
    let first=found.next().ok_or_else(||anyhow::anyhow!("The selected network interface is unavailable. Refresh interfaces and select an available address."))?;
    anyhow::ensure!(
        found.next().is_none(),
        "The selected interface is ambiguous. Select another local address."
    );
    Ok(Some(first))
}

/// Validate current interface availability without binding or changing an existing receiver.
pub(crate) fn validate(
    connection: &ReceiverConnection,
    interfaces: &[InterfaceChoice],
) -> anyhow::Result<()> {
    ReceiverSettings {
        enabled: true,
        connection: connection.clone(),
    }
    .validate()?;
    match connection {
        ReceiverConnection::Multicast { interface, .. } => {
            selected(interface, interfaces)?;
        }
        ReceiverConnection::Unicast { address, .. } => {
            if !address.is_unspecified() {
                anyhow::ensure!(
                    interfaces.iter().any(|entry| entry.address == *address),
                    "The local address is unavailable on this computer."
                );
            }
            scope(*address, interfaces)?;
        }
    }
    Ok(())
}

/// Resolve link-local unicast scope from actual local addresses.
fn scope(address: IpAddr, interfaces: &[InterfaceChoice]) -> anyhow::Result<u32> {
    if let IpAddr::V6(address) = address
        && address.is_unicast_link_local()
    {
        let mut found = interfaces
            .iter()
            .filter(|entry| entry.address == IpAddr::V6(address));
        let first = found
            .next()
            .ok_or_else(|| anyhow::anyhow!("The IPv6 local address is unavailable."))?;
        anyhow::ensure!(
            found.next().is_none(),
            "The IPv6 local address is ambiguous across interfaces."
        );
        return Ok(first.index);
    }
    Ok(0)
}

/// Bind a nonblocking receiver with the selected sharing and membership semantics.
/// Construction errors leave no socket or separate membership registry behind.
pub(crate) fn bind(
    connection: &ReceiverConnection,
    interfaces: &[InterfaceChoice],
) -> anyhow::Result<tokio::net::UdpSocket> {
    validate(connection, interfaces)?;
    let (address, port, multicast) = match connection {
        ReceiverConnection::Unicast { address, port } => (*address, *port, false),
        ReceiverConnection::Multicast { group, port, .. } => (*group, *port, true),
    };
    let socket = Socket::new(
        if address.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        },
        Type::DGRAM,
        Some(Protocol::UDP),
    )?;
    if address.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    if multicast {
        socket.set_reuse_address(true)?;
        #[cfg(target_os = "macos")]
        socket.set_reuse_port(true)?;
        #[cfg(target_os = "linux")]
        if address.is_ipv4() {
            socket.set_multicast_all_v4(false)?;
        } else {
            socket.set_multicast_all_v6(false)?;
        }
    }
    let bind_address = if multicast {
        SocketAddr::new(
            if address.is_ipv4() {
                Ipv4Addr::UNSPECIFIED.into()
            } else {
                Ipv6Addr::UNSPECIFIED.into()
            },
            port,
        )
    } else if let IpAddr::V6(address) = address {
        SocketAddr::V6(SocketAddrV6::new(
            address,
            port,
            0,
            scope(address.into(), interfaces)?,
        ))
    } else {
        SocketAddr::new(address, port)
    };
    socket.bind(&bind_address.into())?;
    if let ReceiverConnection::Multicast {
        group, interface, ..
    } = connection
    {
        let interface = selected(interface, interfaces)?;
        match group {
            IpAddr::V4(group) => {
                let address = match interface.map(|value| value.address) {
                    Some(IpAddr::V4(address)) => address,
                    _ => Ipv4Addr::UNSPECIFIED,
                };
                socket.set_multicast_loop_v4(true)?;
                socket.join_multicast_v4(group, &address)?;
            }
            IpAddr::V6(group) => {
                socket.set_multicast_loop_v6(true)?;
                socket.join_multicast_v6(group, interface.map_or(0, |value| value.index))?;
            }
        }
    }
    socket.set_nonblocking(true)?;
    Ok(tokio::net::UdpSocket::from_std(socket.into())?)
}
