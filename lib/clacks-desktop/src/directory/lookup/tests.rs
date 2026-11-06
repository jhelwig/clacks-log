//! Real provider requests and saves establish keyed sharing and caller-owned application.
use super::runtime::LookupRuntime;
use super::*;
use crate::directory::{
    providers::{self, qrz::QrzSession},
    test_support::{self, fixture, material, response},
};
use clacks_model::{ids::QsoId, scalar::RecordedCallsign};
use std::time::Duration;
use tokio::sync::oneshot;

pub(crate) mod bulk;
mod recovery;
mod saved_results;
mod sharing;
pub(crate) mod support;
use support::{Service, contact, delayed, found, key, login, lookup_count, requests, snapshot};

/// Provider and saved-result scenario shared with configured telemetry tests.
pub(crate) async fn ownership_scenario(
    database: &std::path::Path,
) -> anyhow::Result<(LookupId, QsoId)> {
    let (held, release) = delayed();
    let fixture = fixture(vec![login(), held]).await?;
    let store = ClientStore::create_with_definitions(
        database,
        Default::default(),
        &clacks_model::defaults::account_definitions()?,
        Default::default(),
    )
    .await?;
    let service = Service::from_store(fixture.endpoint.clone(), store, tempfile::tempdir()?)?;
    let owner = QsoId::new();
    let handle = service.client.request(key("OK2CQR"));
    requests(&fixture, 2).await;
    let joined = service.client.request(key("OK2CQR/P"));
    let (changes, mut changed) = watch::channel(());
    let prepared = persistence::prepare(
        service.client.clone(),
        service.store.clone(),
        owner,
        LookupDemand {
            key: key("OK2CQR"),
            due: Instant::now(),
            handle: Some(handle.clone()),
        },
        changes,
    );
    clacks_client_store::save::contacts::save(&service.store, None, contact(owner, "OK2CQR"), None)
        .await?;
    prepared.settle(true);
    release
        .send(include_str!("../providers/fixtures/qrz.xml"))
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), changed.changed()).await??;
    let outcome = handle.wait().await;
    let joined = joined.wait().await;
    let id = snapshot(&outcome).lookup_id();
    assert_eq!(snapshot(&joined).lookup_id(), id);
    assert_eq!(
        snapshot(&joined).lookup_time(),
        snapshot(&outcome).lookup_time()
    );
    let reused = service.client.request(key("OK2CQR/P")).ready().unwrap();
    assert_eq!(snapshot(&reused).lookup_id(), id);
    assert_eq!(
        snapshot(&reused).lookup_time(),
        snapshot(&outcome).lookup_time()
    );
    let row = clacks_client_store::save::load(
        &service.store,
        clacks_sync::record::RecordKey {
            account: service.store.account_id(),
            id: clacks_sync::record::RecordId::Directory(owner),
        },
    )
    .await?
    .unwrap();
    let Some(clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Directory(
        record,
    ))) = row.working_state()
    else {
        panic!("directory snapshot missing")
    };
    assert_eq!(record.snapshot(&key("OK2CQR")).unwrap().lookup_id(), id);
    assert_eq!(lookup_count(&fixture), 1);
    service.close().await?;
    Ok((id, owner))
}
