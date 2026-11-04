//! Real provider failures preserve material-scoped recovery and quiet periods.
use super::*;
#[tokio::test]
async fn directory_lookup_honors_retry_after_without_queuing_automatic_retry() -> anyhow::Result<()>
{
    for short_delay in [None, Some("1")] {
        let mut long = response(429, "");
        long.headers_mut().insert("retry-after", "600".parse()?);
        let mut short = response(429, "");
        if let Some(delay) = short_delay {
            short.headers_mut().insert("retry-after", delay.parse()?);
        }
        let fixture = fixture(vec![login(), long, short, found()]).await?;
        let service = Service::new(fixture.endpoint.clone()).await?;
        // Distinct keys admit concurrent work; matching keys share one operation.
        let first = service.client.request(key("W7FIRST"));
        let second = service.client.request(key("W7SECOND"));
        assert!(matches!(
            first.wait().await.as_ref(),
            LookupState::Failed(_)
        ));
        assert!(matches!(
            second.wait().await.as_ref(),
            LookupState::Failed(_)
        ));
        assert_eq!(lookup_count(&fixture), 2);
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(matches!(
            service.client.retry(key("W7NEXT")).ready().as_deref(),
            Some(LookupState::Failed(_))
        ));
        assert_eq!(
            lookup_count(&fixture),
            2,
            "shorter or absent concurrent limits must not shorten the gate"
        );
        tokio::time::advance(Duration::from_secs(600)).await;
        assert_eq!(
            lookup_count(&fixture),
            2,
            "expiry does not schedule an automatic retry"
        );
        tokio::time::resume();
        assert!(matches!(
            service.client.retry(key("W7NEXT")).wait().await.as_ref(),
            LookupState::Found(_)
        ));
        assert_eq!(lookup_count(&fixture), 3);
        service.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn configuration_changes_preserve_success_but_replace_dispatch_material() -> anyhow::Result<()>
{
    let (held, release) = delayed();
    let old = fixture(vec![login(), found(), held, response(401, "")]).await?;
    let service = Service::new(old.endpoint.clone()).await?;
    let original = service.client.request(key("W7OLD")).wait().await;
    let rejected = service.client.request(key("W7REJECT"));
    requests(&old, 3).await;
    let new = fixture(vec![login(), found(), found()]).await?;
    let mut replacement = material();
    replacement.login.password = "replacement-fixture-secret".into();
    {
        let mut state = service.client.inner.state.lock().unwrap();
        state.selected = Ok(Some(Selected {
            material: replacement.clone(),
            provider: clacks_model::directory::values::DirectoryProvider::Qrz,
            session: ProviderSession::Qrz(QrzSession::new(
                providers::client()?,
                new.endpoint.clone(),
                replacement,
            )),
            paused: false,
            retry_after: None,
        }));
    }
    let retained = service.client.request(key("W7OLD")).ready().unwrap();
    assert_eq!(
        snapshot(&original).lookup_id(),
        snapshot(&retained).lookup_id()
    );
    assert_eq!(
        snapshot(&original).lookup_time(),
        snapshot(&retained).lookup_time()
    );
    let fresh = service.client.request(key("W7NEW")).wait().await;
    assert!(matches!(fresh.as_ref(), LookupState::Found(_)));
    release
        .send("<QRZDatabase><Session><Error>Session Timeout</Error></Session></QRZDatabase>")
        .unwrap();
    assert!(matches!(
        rejected.wait().await.as_ref(),
        LookupState::PausedAuthentication
    ));
    assert!(
        matches!(
            service.client.request(key("W7AFTER")).wait().await.as_ref(),
            LookupState::Found(_)
        ),
        "a late rejection must not pause replacement credentials"
    );
    assert_eq!(lookup_count(&new), 2);
    service.close().await
}
