//! Production UDP capture with a real, controlled directory adapter and local store.
use crate::{
    directory::{
        lookup::tests::support::{Service, delayed, login, requests},
        test_support::fixture,
    },
    ws_suite::{self, Outputs, ReceiverView, ReceptionState},
};
use clacks_client_store::{
    store::ClientStore,
    ws_suite::{ReceiverConnection, ReceiverSettings},
};
use clacks_model::{contact::Qso, ids::QsoId};
use clacks_sync::record::{RecordId, RecordKey};
use std::{net::SocketAddr, time::Duration};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

struct Receiver {
    address: SocketAddr,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<anyhow::Result<ws_suite::pairing::Pairing>>,
    changes: watch::Receiver<()>,
    view: watch::Receiver<ReceiverView>,
    _commands: mpsc::Sender<ws_suite::ReceiverRequest>,
}
impl Receiver {
    async fn start(service: &Service) -> anyhow::Result<Self> {
        let probe = std::net::UdpSocket::bind("127.0.0.1:0")?;
        let address = probe.local_addr()?;
        drop(probe);
        clacks_client_store::ws_suite::save(
            &service.store,
            &ReceiverSettings {
                enabled: true,
                connection: ReceiverConnection::Unicast {
                    address: address.ip(),
                    port: address.port(),
                },
            },
        )
        .await?;
        let (updates, mut view) = watch::channel(ReceiverView::default());
        let (changes, changed) = watch::channel(());
        let (commands, input) = mpsc::channel(8);
        let stop = CancellationToken::new();
        let task = tokio::spawn(ws_suite::runtime::run(
            crate::ws_suite::runtime::Resources {
                geography: watch::channel(Default::default()).1,
                store: service.store.clone(),
                lookup: service.client.clone(),
                settings_updates: watch::channel(None).0,
                changes,
            },
            input,
            Outputs {
                calls: mpsc::unbounded_channel().0,
                updates,
                context: Default::default(),
            },
            Default::default(),
            stop.clone(),
        ));
        tokio::time::timeout(Duration::from_secs(5), async {
            while view.borrow().reception != ReceptionState::Listening {
                view.changed().await?;
            }
            anyhow::Ok(())
        })
        .await??;
        Ok(Self {
            address,
            stop,
            task,
            changes: changed,
            view,
            _commands: commands,
        })
    }
    async fn send(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
        sender.send_to(bytes, self.address).await?;
        Ok(())
    }
    async fn saved(&mut self, store: &ClientStore, count: usize) -> anyhow::Result<Vec<Qso>> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let mut read = clacks_client_store::read::snapshot(store).await?;
                let page = read.contact_page(&Default::default()).await?;
                read.close().await?;
                if page.rows.len() == count {
                    return Ok(page.rows.into_iter().map(|row| row.working.qso).collect());
                }
                self.changes.changed().await?;
            }
        })
        .await?
    }
    async fn close(self) -> anyhow::Result<()> {
        self.stop.cancel();
        tokio::time::timeout(Duration::from_secs(5), self.task).await???;
        Ok(())
    }
}
async fn snapshot(
    store: &ClientStore,
    qso: QsoId,
) -> anyhow::Result<Option<clacks_sync::client::ClientRecord>> {
    clacks_client_store::save::load(
        store,
        RecordKey {
            account: store.account_id(),
            id: RecordId::Directory(qso),
        },
    )
    .await
}
#[tokio::test]
async fn capture_miss_saves_before_directory_response() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let provider = fixture(vec![login(), held]).await?;
    let service = Service::new(provider.endpoint.clone()).await?;
    let mut receiver = Receiver::start(&service).await?;
    receiver
        .send(&crate::test_support::ws_suite::adif("OK2CQR"))
        .await?;
    let saved = receiver.saved(&service.store, 1).await?;
    requests(&provider, 2).await;
    assert_eq!(provider.lookup_count(), 1);
    assert!(snapshot(&service.store, saved[0].id()).await?.is_none());
    release
        .send(include_str!(
            "../../../directory/providers/fixtures/qrz.xml"
        ))
        .unwrap();
    service.settled().await;
    assert!(snapshot(&service.store, saved[0].id()).await?.is_some());
    assert!(receiver.view.borrow().capture_notice.is_none());
    receiver.close().await?;
    service.close().await
}

#[tokio::test]
async fn capture_reuses_or_joins_entry_lookup() -> anyhow::Result<()> {
    use crate::directory::lookup::{LookupState, tests::support::key};
    use clacks_sync::record::{RecordState, RecordValue};
    for cached in [false, true] {
        let (held, release) = delayed();
        let provider = fixture(vec![login(), held]).await?;
        let service = Service::new(provider.endpoint.clone()).await?;
        let handle = service.client.request(key("OK2CQR"));
        requests(&provider, 2).await;
        let mut release = Some(release);
        if cached {
            release
                .take()
                .unwrap()
                .send(include_str!(
                    "../../../directory/providers/fixtures/qrz.xml"
                ))
                .unwrap();
            handle.clone().wait().await;
        }
        let mut receiver = Receiver::start(&service).await?;
        receiver
            .send(&crate::test_support::ws_suite::adif("OK2CQR/P"))
            .await?;
        let saved = receiver.saved(&service.store, 1).await?;
        if !cached {
            assert!(snapshot(&service.store, saved[0].id()).await?.is_none());
        }
        // The display consumer can release its interest; capture independently retains it.
        let shared = handle.clone();
        drop(handle);
        if let Some(release) = release {
            release
                .send(include_str!(
                    "../../../directory/providers/fixtures/qrz.xml"
                ))
                .unwrap();
        }
        let result = shared.wait().await;
        service.settled().await;
        let LookupState::Found(expected) = result.as_ref() else {
            panic!("found")
        };
        let row = snapshot(&service.store, saved[0].id()).await?.unwrap();
        let Some(RecordState::Live(RecordValue::Directory(record))) = row.working_state() else {
            panic!("directory")
        };
        let actual = record.snapshot(&key("OK2CQR")).unwrap();
        assert_eq!(actual.lookup_id(), expected.lookup_id());
        assert_eq!(actual.lookup_time(), expected.lookup_time());
        assert_eq!(provider.lookup_count(), 1);
        receiver.close().await?;
        service.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn distinct_captures_share_station_information() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let provider = fixture(vec![login(), held]).await?;
    let service = Service::new(provider.endpoint.clone()).await?;
    let mut receiver = Receiver::start(&service).await?;
    receiver
        .send(&crate::test_support::ws_suite::adif("OK2CQR"))
        .await?;
    receiver.saved(&service.store, 1).await?;
    requests(&provider, 2).await;
    receiver
        .send(&crate::test_support::ws_suite::adif_at(
            "OK2CQR/P", "000001",
        ))
        .await?;
    let contacts = receiver.saved(&service.store, 2).await?;
    assert_ne!(contacts[0].id(), contacts[1].id());
    release
        .send(include_str!(
            "../../../directory/providers/fixtures/qrz.xml"
        ))
        .unwrap();
    service.settled().await;
    for contact in contacts {
        assert!(snapshot(&service.store, contact.id()).await?.is_some());
    }
    assert_eq!(provider.lookup_count(), 1);
    receiver.close().await?;
    service.close().await
}

#[tokio::test]
async fn late_capture_lookup_respects_current_record() -> anyhow::Result<()> {
    use clacks_client_store::save::{self, SaveOutcome};
    use clacks_sync::record::RecordState;
    for edit in ["delete", "call", "comment"] {
        let (held, release) = delayed();
        let provider = fixture(vec![login(), held]).await?;
        let service = Service::new(provider.endpoint.clone()).await?;
        let mut receiver = Receiver::start(&service).await?;
        receiver
            .send(&crate::test_support::ws_suite::adif("OK2CQR"))
            .await?;
        let mut qso = receiver.saved(&service.store, 1).await?.remove(0);
        requests(&provider, 2).await;
        let basis = save::load(
            &service.store,
            RecordKey {
                account: service.store.account_id(),
                id: RecordId::Qso(qso.id()),
            },
        )
        .await?
        .unwrap();
        if edit == "delete" {
            save::save(
                &service.store,
                basis.key(),
                Some(&basis),
                RecordState::Deleted,
            )
            .await?;
        } else {
            crate::test_support::edit(&mut qso, |fields| {
                if edit == "call" {
                    fields.contacted_station.callsign =
                        clacks_model::scalar::RecordedCallsign::new("W7OTHER")?;
                } else {
                    fields.comment = Some("Retain my correction".into());
                }
                Ok(())
            });
            assert!(matches!(
                save::contacts::save(&service.store, Some(&basis), qso.clone(), None).await?,
                SaveOutcome::Saved(_)
            ));
        }
        release
            .send(include_str!(
                "../../../directory/providers/fixtures/qrz.xml"
            ))
            .unwrap();
        service.settled().await;
        assert_eq!(
            snapshot(&service.store, qso.id()).await?.is_some(),
            edit == "comment"
        );
        if edit == "comment" {
            assert_eq!(receiver.saved(&service.store, 1).await?[0], qso);
        }
        receiver.close().await?;
        service.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn capture_lookup_failure_keeps_saved_qso() -> anyhow::Result<()> {
    let provider = fixture(vec![
        login(),
        crate::directory::test_support::response(503, "unavailable"),
    ])
    .await?;
    let service = Service::new(provider.endpoint.clone()).await?;
    let mut receiver = Receiver::start(&service).await?;
    receiver
        .send(&crate::test_support::ws_suite::adif("OK2CQR"))
        .await?;
    let qso = receiver.saved(&service.store, 1).await?.remove(0);
    service.settled().await;
    assert!(snapshot(&service.store, qso.id()).await?.is_none());
    assert!(receiver.view.borrow().capture_notice.is_none());
    assert_eq!(provider.lookup_count(), 1);
    receiver.close().await?;
    service.close().await
}

#[tokio::test]
async fn receiver_drain_does_not_wait_for_provider() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let provider = fixture(vec![login(), held]).await?;
    let service = Service::new(provider.endpoint.clone()).await?;
    let mut receiver = Receiver::start(&service).await?;
    receiver
        .send(&crate::test_support::ws_suite::adif("OK2CQR"))
        .await?;
    let qso = receiver.saved(&service.store, 1).await?.remove(0);
    requests(&provider, 2).await;
    let (reply, response) = tokio::sync::oneshot::channel();
    let mut settings = receiver.view.borrow().settings.clone().unwrap();
    settings.enabled = false;
    receiver
        ._commands
        .send(ws_suite::ReceiverRequest {
            action: ws_suite::ReceiverAction::Apply { next: settings },
            reply,
        })
        .await?;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), response).await??,
        ws_suite::ReceiverResponse::Applied(_)
    ));
    receiver.close().await?;
    assert!(snapshot(&service.store, qso.id()).await?.is_none());
    // Workspace shutdown cancels the held provider and joins all owned work.
    tokio::time::timeout(Duration::from_secs(5), service.close()).await??;
    drop(release);
    Ok(())
}
