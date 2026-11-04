//! Real socket cases detect missing membership, sharing, interface and local delivery wiring.
use super::super::{Outputs, ReceiverView, ReceptionState, interfaces, runtime, socket};
use clacks_client_store::{
    store::ClientStore,
    ws_suite::{self, InterfaceSelection, ReceiverConnection, ReceiverSettings},
};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    time::Duration,
};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn multicast_coexists_with_peer_receiver() -> anyhow::Result<()> {
    let choices = interfaces::enumerate()?;
    let interface = choices
        .iter()
        .find(|entry| entry.address == IpAddr::V4(Ipv4Addr::LOCALHOST))
        .ok_or_else(|| anyhow::anyhow!("IPv4 loopback interface required"))?;
    multicast_capture(
        interface,
        &choices,
        Ipv4Addr::new(239, 255, 19, 19).into(),
        Ipv4Addr::new(239, 255, 19, 20).into(),
    )
    .await
}

#[tokio::test]
async fn ipv6_multicast_socket_configuration() -> anyhow::Result<()> {
    let choices = interfaces::enumerate()?;
    let interface = choices
        .iter()
        .find(|entry| entry.address == IpAddr::V6(Ipv6Addr::LOCALHOST))
        .ok_or_else(|| {
            anyhow::anyhow!("IPv6 loopback interface required for socket construction")
        })?;
    let probe = std::net::UdpSocket::bind("[::1]:0")?;
    let port = probe.local_addr()?.port();
    drop(probe);

    let receiver = socket::bind(
        &ReceiverConnection::Multicast {
            group: "ff01::114".parse()?,
            port,
            interface: InterfaceSelection::Explicit {
                name: interface.name.clone(),
                address: interface.address,
            },
        },
        &choices,
    )?
    .into_std()?;
    let options = socket2::SockRef::from(&receiver);

    assert_eq!(
        receiver.local_addr()?,
        SocketAddr::from((Ipv6Addr::UNSPECIFIED, port))
    );
    assert!(options.only_v6()?);
    assert!(options.reuse_address()?);
    assert!(options.multicast_loop_v6()?);
    #[cfg(target_os = "linux")]
    assert!(!options.multicast_all_v6()?);
    Ok(())
}

#[tokio::test]
#[ignore = "native IPv6 packet verification requires an existing usable IPv6 multicast interface; run explicitly with --ignored --nocapture"]
async fn ipv6_multicast_capture_on_existing_interface() -> anyhow::Result<()> {
    use network_interface::{NetworkInterface, NetworkInterfaceConfig};

    let choices = interfaces::enumerate()?;
    let adapters = NetworkInterface::show()?;
    let interface = adapters
        .iter()
        .filter(|adapter| !adapter.internal)
        .find_map(|adapter| {
            choices.iter().find(|entry| {
                entry.index == adapter.index
                    && matches!(entry.address, IpAddr::V6(address) if !address.is_unspecified() && !address.is_loopback())
            })
        })
        .ok_or_else(|| anyhow::anyhow!(
            "IPv6 multicast verification incomplete: no existing non-loopback IPv6 interface is available"
        ))?;
    eprintln!(
        "IPv6 multicast verification on {} ({})",
        interface.name, interface.address
    );

    // Interface-local scope and zero hops keep test traffic on this computer.
    multicast_capture(
        interface,
        &choices,
        "ff11::19:19".parse()?,
        "ff11::19:20".parse()?,
    )
    .await
    .map_err(|error| {
        anyhow::anyhow!(
            "IPv6 multicast verification failed on {} ({}): {error:#}",
            interface.name,
            interface.address
        )
    })
}

async fn multicast_capture(
    interface: &interfaces::InterfaceChoice,
    choices: &[interfaces::InterfaceChoice],
    group: IpAddr,
    other: IpAddr,
) -> anyhow::Result<()> {
    let local = match interface.address {
        IpAddr::V4(address) => SocketAddr::from((address, 0)),
        IpAddr::V6(address) => SocketAddrV6::new(address, 0, 0, interface.index).into(),
    };
    let probe = std::net::UdpSocket::bind(local)?;
    let port = probe.local_addr()?.port();
    drop(probe);
    let connection = ReceiverConnection::Multicast {
        group,
        port,
        interface: InterfaceSelection::Explicit {
            name: interface.name.clone(),
            address: interface.address,
        },
    };
    let peer = socket::bind(&connection, choices)?;
    let other_peer = socket::bind(
        &ReceiverConnection::Multicast {
            group: other,
            port,
            interface: InterfaceSelection::Explicit {
                name: interface.name.clone(),
                address: interface.address,
            },
        },
        choices,
    )?;
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        Default::default(),
        Default::default(),
    )
    .await?;
    ws_suite::save(
        &store,
        &ReceiverSettings {
            enabled: true,
            connection,
        },
    )
    .await?;
    let (updates, mut view) = watch::channel(ReceiverView::default());
    let (commands, input) = mpsc::channel(8);
    let stop = CancellationToken::new();
    let task = tokio::spawn(runtime::run(
        store.clone(),
        input,
        Outputs {
            calls: mpsc::unbounded_channel().0,
            updates,
            context: Default::default(),
        },
        watch::channel(None).0,
        watch::channel(()).0,
        Default::default(),
        stop.clone(),
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if view.borrow().settings.is_some() {
                break;
            }
            view.changed().await?;
        }
        anyhow::Ok(())
    })
    .await??;
    assert_eq!(view.borrow().reception, ReceptionState::Listening);

    let sender = socket2::Socket::new(
        if group.is_ipv4() {
            socket2::Domain::IPV4
        } else {
            socket2::Domain::IPV6
        },
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    match interface.address {
        IpAddr::V4(address) => {
            sender.set_multicast_if_v4(&address)?;
            sender.set_multicast_loop_v4(true)?;
        }
        IpAddr::V6(_) => {
            sender.set_multicast_if_v6(interface.index)?;
            sender.set_multicast_loop_v6(true)?;
            sender.set_multicast_hops_v6(0)?;
        }
    }
    sender.bind(&local.into())?;
    let destination = |address| match address {
        IpAddr::V4(address) => SocketAddr::from((address, port)),
        IpAddr::V6(address) => SocketAddrV6::new(address, port, 0, interface.index).into(),
    };
    let other_packet = crate::test_support::ws_suite::adif("W7XYZ");
    sender.send_to(&other_packet, &destination(other).into())?;
    let first = crate::test_support::ws_suite::structured("W7HEX");
    let second = crate::test_support::ws_suite::adif("W7HEX");
    sender.send_to(&first, &destination(group).into())?;
    sender.send_to(&second, &destination(group).into())?;

    let mut buffer = vec![0; 65535];
    let (length, _) =
        tokio::time::timeout(Duration::from_secs(3), other_peer.recv_from(&mut buffer)).await??;
    assert_eq!(buffer.get(..length), Some(other_packet.as_slice()));
    for expected in [&first, &second] {
        let (length, _) =
            tokio::time::timeout(Duration::from_secs(3), peer.recv_from(&mut buffer)).await??;
        assert_eq!(buffer.get(..length), Some(expected.as_slice()));
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let mut read = clacks_client_store::read::snapshot(&store).await?;
            let page = read.contact_page(&Default::default()).await?;
            read.close().await?;
            if !page.rows.is_empty() {
                break anyhow::Ok(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    stop.cancel();
    tokio::time::timeout(Duration::from_secs(5), task).await???;
    drop(commands);

    let mut read = clacks_client_store::read::snapshot(&store).await?;
    let page = read.contact_page(&Default::default()).await?;
    read.close().await?;
    assert_eq!(page.rows.len(), 1);
    let contact = &page.rows.first().unwrap().working.qso;
    assert_eq!(contact.contacted_station().callsign.as_str(), "W7HEX");
    assert_eq!(contact.comment().as_deref(), Some("packet-private!"));

    store.close().await?;
    Ok(())
}
