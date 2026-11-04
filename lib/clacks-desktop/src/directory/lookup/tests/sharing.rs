//! Keyed sharing, retention and eviction through actual provider HTTP.
use super::*;
#[tokio::test]
async fn shared_lookup_joins_and_reuses_one_result() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let fixture = fixture(vec![login(), held]).await?;
    let service = Service::new(fixture.endpoint.clone()).await?;
    let first = service.client.request(key("OK2CQR"));
    requests(&fixture, 2).await;
    let joined = service.client.request(key("OK2CQR/P"));
    release
        .send(include_str!("../../providers/fixtures/qrz.xml"))
        .unwrap();
    let first = first.wait().await;
    let joined = joined.wait().await;
    let reused = service.client.request(key("OK2CQR"));
    let reused = reused
        .ready()
        .expect("cache success is immediately available");
    assert_eq!(lookup_count(&fixture), 1);
    assert_eq!(snapshot(&first).lookup_id(), snapshot(&joined).lookup_id());
    assert_eq!(
        snapshot(&first).lookup_time(),
        snapshot(&reused).lookup_time()
    );
    assert_eq!(snapshot(&first).lookup_id(), snapshot(&reused).lookup_id());
    service.close().await
}

#[tokio::test]
async fn shared_lookup_expires_and_evicts_successes() -> anyhow::Result<()> {
    let fixture = fixture(
        std::iter::once(login())
            .chain((0..132).map(|_| found()))
            .collect(),
    )
    .await?;
    let service = Service::new(fixture.endpoint.clone()).await?;
    let original = service.client.request(key("W7OLD")).wait().await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(23 * 3600)).await;
    assert!(service.client.request(key("W7OLD")).ready().is_some());
    tokio::time::advance(Duration::from_secs(3600)).await;
    let expired = service.client.request(key("W7OLD"));
    assert!(
        expired.ready().is_none(),
        "access must not extend the 24-hour lifetime"
    );
    tokio::time::resume();
    let refreshed = expired.wait().await;
    assert_ne!(
        snapshot(&original).lookup_id(),
        snapshot(&refreshed).lookup_id()
    );
    for index in 0..127 {
        service
            .client
            .request(key(&format!("K7X{index}")))
            .wait()
            .await;
    }
    // Protect W7OLD by accessing it, making K7X0 the oldest of 128 successes.
    service.client.request(key("W7OLD"));
    service.client.request(key("W7NEW")).wait().await;
    let evicted = service.client.request(key("K7X0"));
    assert!(
        evicted.ready().is_none(),
        "success 129 must evict the least recently used result"
    );
    evicted.wait().await;
    assert_eq!(
        snapshot(&original).key(),
        &key("W7OLD"),
        "held handles survive eviction/expiry"
    );
    assert_eq!(lookup_count(&fixture), 131);
    service.close().await
}
