//! Real receiver preference and listener ownership through workspace admission.
use super::*;
use crate::ws_suite::{ReceiverAction, ReceiverRequest, ReceiverResponse, ReceptionState};
use clacks_client_store::ws_suite::{ReceiverConnection, ReceiverSettings};

async fn apply(
    commands: &mpsc::Sender<SessionCommand>,
    next: ReceiverSettings,
) -> anyhow::Result<ReceiverResponse> {
    let (reply, result) = tokio::sync::oneshot::channel();
    commands
        .send(SessionCommand::WsSuite(ReceiverRequest {
            action: ReceiverAction::Apply { next },
            reply,
        }))
        .await?;
    Ok(tokio::time::timeout(Duration::from_secs(10), result).await??)
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_settings_survive_bind_failure_and_retry() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let conflict = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let endpoint = conflict.local_addr()?;
    let next = ReceiverSettings {
        enabled: true,
        connection: ReceiverConnection::Unicast {
            address: endpoint.ip(),
            port: endpoint.port(),
        },
    };
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Ready { .. })
    })
    .await?;

    let response = apply(&fixture.commands, next.clone()).await?;
    let ReceiverResponse::Applied(view) = response else {
        panic!("{response:?}")
    };
    assert_eq!(view.settings, Some(next.clone()));
    assert!(matches!(view.reception, ReceptionState::Failed(_)));
    drop(conflict);
    let response = apply(&fixture.commands, next.clone()).await?;
    let ReceiverResponse::Applied(view) = response else {
        panic!("{response:?}")
    };
    assert!(matches!(view.reception, ReceptionState::Listening));
    assert!(std::net::UdpSocket::bind(endpoint).is_err());
    let mut off = next;
    off.enabled = false;
    let ReceiverResponse::Applied(view) = apply(&fixture.commands, off).await? else {
        panic!("Applied")
    };
    assert!(matches!(view.reception, ReceptionState::Off));
    assert!(std::net::UdpSocket::bind(endpoint).is_ok());
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    Ok(())
}

async fn contacts(
    store: &clacks_client_store::store::ClientStore,
) -> anyhow::Result<Vec<clacks_model::contact::Qso>> {
    let mut read = clacks_client_store::read::snapshot(store).await?;
    let page = read.contact_page(&Default::default()).await?;
    read.close().await?;
    Ok(page.rows.into_iter().map(|row| row.working.qso).collect())
}

#[tokio::test(flavor = "multi_thread")]
async fn udp_pair_saves_one_local_contact() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let database = fixture.resources.paths.database.clone();
    let probe = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let endpoint = probe.local_addr()?;
    drop(probe);
    let next = ReceiverSettings {
        enabled: true,
        connection: ReceiverConnection::Unicast {
            address: endpoint.ip(),
            port: endpoint.port(),
        },
    };
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Ready { .. })
    })
    .await?;
    let response = apply(&fixture.commands, next.clone()).await?;
    let ReceiverResponse::Applied(view) = response else {
        panic!("{response:?}")
    };
    assert_eq!(view.reception, ReceptionState::Listening);
    let store =
        clacks_client_store::store::ClientStore::open_discovered(&database, Default::default())
            .await?;
    let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
    let packet = crate::test_support::ws_suite::adif("W7HEX");
    sender
        .send_to(
            &crate::test_support::ws_suite::structured("W7HEX"),
            endpoint,
        )
        .await?;
    sender.send_to(&packet, endpoint).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let rows = contacts(&store).await?;
            if let Some(qso) = rows.into_iter().next() {
                break anyhow::Ok(qso);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    sender.send_to(&packet, endpoint).await?;
    sender.send_to(&packet, endpoint).await?;
    // Reconfiguration settles admitted work and retains latest completion identity.
    let probe = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let second = probe.local_addr()?;
    drop(probe);
    let changed = ReceiverSettings {
        enabled: true,
        connection: ReceiverConnection::Unicast {
            address: second.ip(),
            port: second.port(),
        },
    };
    let ReceiverResponse::Applied(view) = apply(&fixture.commands, changed).await? else {
        panic!("Applied")
    };
    assert_eq!(view.reception, ReceptionState::Listening);
    sender.send_to(&packet, second).await?;
    let distinct = crate::test_support::ws_suite::adif("W7XYZ");
    sender.send_to(&distinct, second).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if contacts(&store).await?.len() == 2 {
                break anyhow::Ok(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    let rows = contacts(&store).await?;
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|qso| qso.comment().as_deref() == Some("packet-private!"))
    );
    assert!(
        rows.iter()
            .all(|qso| qso.own_context().transmit_power.unwrap().get() == 5000)
    );
    assert!(
        rows.iter()
            .all(|qso| qso.own_station().callsign.as_str() == "W7ABC")
    );
    store.close().await?;
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    let store =
        clacks_client_store::store::ClientStore::open_discovered(&database, Default::default())
            .await?;
    assert_eq!(contacts(&store).await?.len(), 2);
    store.close().await?;
    Ok(())
}

/// Observe actual admission without adding a production pause/failpoint API.
#[derive(Clone, Default)]
struct AdmissionSignal {
    /// Wake the scenario at the real receiver admission event.
    notify: std::sync::Arc<tokio::sync::Notify>,
    /// Establish that the close request occurs inside the original pairing window.
    at: std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>,
    /// Completed capture must precede starting this session's identity-changing workflow.
    committed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Detect removing the session's drain before its enrollment worker.
    early_enrollment: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl<S> tracing_subscriber::Layer<S> for AdmissionSignal
where
    S: tracing::Subscriber,
{
    fn on_new_span(
        &self,
        attributes: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _context: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if attributes.metadata().name() == "desktop.enrollment.workflow"
            && self.at.lock().unwrap().is_some()
            && !self.committed.load(std::sync::atomic::Ordering::SeqCst)
        {
            self.early_enrollment
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _context: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().name() == "ws_suite.capture_committed" {
            self.committed
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if event.metadata().name() == "ws_suite.capture_admitted" {
            *self.at.lock().unwrap() = Some(std::time::Instant::now());
            self.notify.notify_one();
        }
    }
}

/// Run the observer-based workflow under one subscriber shared by every worker.
/// The existing composition harness gives this subscriber its own process.
pub(crate) fn run_isolated(case: &str) -> anyhow::Result<()> {
    use tracing_subscriber::prelude::*;
    let signal = AdmissionSignal::default();
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(signal.clone()))?;

    let mut builder = if case == "ws-suite-session-queued" {
        tokio::runtime::Builder::new_current_thread()
    } else {
        tokio::runtime::Builder::new_multi_thread()
    };
    builder.worker_threads(2);
    let runtime = builder.enable_all().build()?;
    let result = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(30), async {
            match case {
                "ws-suite-session-close" => close_drains_scenario(signal).await,
                "ws-suite-session-enrollment" => enrollment_drains_scenario(signal).await,
                "ws-suite-session-queued" => queued_counterpart_scenario(signal).await,
                "ws-suite-session-reconfigure" => receiver_change_scenario(signal).await,
                _ => anyhow::bail!("Unknown WS Suite session scenario: {case}"),
            }
        })
        .await
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    result.map_err(|error| anyhow::anyhow!("WS Suite session scenario {case}: {error}"))?
}

#[test]
fn close_drains_an_admitted_single_representation() -> anyhow::Result<()> {
    crate::telemetry::tests::child_case("ws-suite-session-close")
}

async fn close_drains_scenario(signal: AdmissionSignal) -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let database = fixture.resources.paths.database.clone();
    let probe = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let endpoint = probe.local_addr()?;
    drop(probe);
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    let response = apply(
        &fixture.commands,
        ReceiverSettings {
            enabled: true,
            connection: ReceiverConnection::Unicast {
                address: endpoint.ip(),
                port: endpoint.port(),
            },
        },
    )
    .await?;
    assert!(
        matches!(response,ReceiverResponse::Applied(ref view) if view.reception==ReceptionState::Listening)
    );
    let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
    sender
        .send_to(&crate::test_support::ws_suite::adif("W7HEX"), endpoint)
        .await?;
    tokio::time::timeout(Duration::from_secs(5), signal.notify.notified()).await?;
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    let closing = next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Closing { .. } | SessionStatus::Closed)
    })
    .await?;
    assert!(matches!(closing, SessionStatus::Closing { .. }));
    assert!(
        signal.at.lock().unwrap().unwrap().elapsed() < Duration::from_millis(100),
        "close must begin while the single representation is pending"
    );
    task.await??;
    let store =
        clacks_client_store::store::ClientStore::open_discovered(&database, Default::default())
            .await?;
    assert_eq!(contacts(&store).await?.len(), 1);
    store.close().await?;
    Ok(())
}

#[test]
fn enrollment_drains_capture_before_account_eligibility() -> anyhow::Result<()> {
    crate::telemetry::tests::child_case("ws-suite-session-enrollment")
}

async fn enrollment_drains_scenario(signal: AdmissionSignal) -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let database = fixture.resources.paths.database.clone();
    let mut enrollment = fixture.resources.enrollment_updates.subscribe();
    let probe = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let endpoint = probe.local_addr()?;
    drop(probe);
    let origin = std::net::TcpListener::bind("127.0.0.1:0")?;
    let server = format!("http://{}", origin.local_addr()?);
    drop(origin);
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    apply(
        &fixture.commands,
        ReceiverSettings {
            enabled: true,
            connection: ReceiverConnection::Unicast {
                address: endpoint.ip(),
                port: endpoint.port(),
            },
        },
    )
    .await?;
    let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
    sender
        .send_to(&crate::test_support::ws_suite::adif("W7HEX"), endpoint)
        .await?;
    tokio::time::timeout(Duration::from_secs(5), signal.notify.notified()).await?;
    fixture
        .commands
        .send(SessionCommand::Enrollment(
            crate::enrollment::EnrollmentCommand::Connect {
                server,
                label: "Desktop".into(),
            },
        ))
        .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if matches!(
                *enrollment.borrow(),
                crate::enrollment::EnrollmentStatus::Failed { .. }
            ) {
                break;
            }
            enrollment.changed().await?;
        }
        anyhow::Ok(())
    })
    .await??;
    let store =
        clacks_client_store::store::ClientStore::open_discovered(&database, Default::default())
            .await?;
    assert!(
        !signal
            .early_enrollment
            .load(std::sync::atomic::Ordering::SeqCst),
        "the identity-changing worker must start after the admitted capture drains"
    );
    assert_eq!(
        contacts(&store).await?.len(),
        1,
        "capture must settle before first-enrollment eligibility is checked"
    );
    assert!(
        clacks_client_store::enrollment::attempt(&store)
            .await?
            .is_none(),
        "the admitted contact makes this standalone workspace ineligible before any authorization attempt"
    );
    // A failed connection resumes the same workspace's reception and recognition.
    let response = apply(
        &fixture.commands,
        clacks_client_store::ws_suite::load(&store).await?,
    )
    .await?;
    assert!(
        matches!(response,ReceiverResponse::Applied(ref view) if view.reception==ReceptionState::Listening)
    );
    store.close().await?;
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    Ok(())
}

#[test]
fn queued_counterpart_precedes_expiration_after_scheduler_pause() -> anyhow::Result<()> {
    crate::telemetry::tests::child_case("ws-suite-session-queued")
}

async fn queued_counterpart_scenario(signal: AdmissionSignal) -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let database = fixture.resources.paths.database.clone();
    let probe = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let endpoint = probe.local_addr()?;
    drop(probe);
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    apply(
        &fixture.commands,
        ReceiverSettings {
            enabled: true,
            connection: ReceiverConnection::Unicast {
                address: endpoint.ip(),
                port: endpoint.port(),
            },
        },
    )
    .await?;
    let sender = std::net::UdpSocket::bind("127.0.0.1:0")?;
    sender.send_to(
        &crate::test_support::ws_suite::structured("W7HEX"),
        endpoint,
    )?;
    tokio::time::timeout(Duration::from_secs(5), signal.notify.notified()).await?;
    assert!(
        signal.at.lock().unwrap().unwrap().elapsed() < Duration::from_millis(100),
        "the scheduler pause must begin within the pairing window"
    );

    // Pause this single-thread runtime while the OS queues the counterpart before
    // expiry. At the next poll both socket readiness and the timer are ready.
    let queued = std::thread::spawn(move || -> std::io::Result<()> {
        std::thread::sleep(Duration::from_millis(20));
        sender.send_to(&crate::test_support::ws_suite::adif("W7HEX"), endpoint)?;
        Ok(())
    });
    std::thread::sleep(Duration::from_millis(150));
    queued.join().unwrap()?;
    let store =
        clacks_client_store::store::ClientStore::open_discovered(&database, Default::default())
            .await?;
    let qso = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(qso) = contacts(&store).await?.into_iter().next() {
                break anyhow::Ok(qso);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    assert_eq!(qso.comment().as_deref(), Some("packet-private!"));
    store.close().await?;
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    Ok(())
}

#[test]
fn receiver_change_drains_and_retains_recent_identity() -> anyhow::Result<()> {
    crate::telemetry::tests::child_case("ws-suite-session-reconfigure")
}

async fn receiver_change_scenario(signal: AdmissionSignal) -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let database = fixture.resources.paths.database.clone();
    let first = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let endpoint = first.local_addr()?;
    drop(first);
    let second = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let replacement = second.local_addr()?;
    drop(second);
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    let settings = |endpoint: std::net::SocketAddr| ReceiverSettings {
        enabled: true,
        connection: ReceiverConnection::Unicast {
            address: endpoint.ip(),
            port: endpoint.port(),
        },
    };
    apply(&fixture.commands, settings(endpoint)).await?;
    let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
    sender
        .send_to(&crate::test_support::ws_suite::adif("W7HEX"), endpoint)
        .await?;
    tokio::time::timeout(Duration::from_secs(5), signal.notify.notified()).await?;
    assert!(signal.at.lock().unwrap().unwrap().elapsed() < Duration::from_millis(100));
    let response = apply(&fixture.commands, settings(replacement)).await?;
    assert!(
        matches!(response,ReceiverResponse::Applied(ref view) if view.reception==ReceptionState::Listening)
    );
    let store =
        clacks_client_store::store::ClientStore::open_discovered(&database, Default::default())
            .await?;
    assert_eq!(contacts(&store).await?.len(), 1);
    sender
        .send_to(
            &crate::test_support::ws_suite::structured("W7HEX"),
            replacement,
        )
        .await?;
    sender
        .send_to(&crate::test_support::ws_suite::adif("W7HEX"), replacement)
        .await?;
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(10), contacts(&store))
            .await??
            .len(),
        1
    );
    tokio::time::timeout(Duration::from_secs(10), store.close()).await??;
    Ok(())
}
