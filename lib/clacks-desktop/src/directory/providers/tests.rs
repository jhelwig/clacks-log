//! Authored HTTP fixtures verify Clacks request composition and result interpretation.

use super::{
    ProviderError, ProviderLookup, SafeProviderFailure, client, hamqth::HamQthSession,
    qrz::QrzSession,
};
use clacks_model::position::Position;
use std::time::Duration;

use super::super::test_support::{fixture, key, material, response};
use axum::{body::Body, response::Response};

#[tokio::test]
async fn directory_qrz_retains_concurrent_rotations_without_stale_expiry_clearing_them()
-> anyhow::Result<()> {
    /// Hold response decoding until the caller releases the authored body.
    fn delayed() -> (tokio::sync::oneshot::Sender<&'static str>, Response) {
        let (release, released) = tokio::sync::oneshot::channel::<&'static str>();
        let body = Body::from_stream(futures_util::stream::once(async {
            Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(released.await.unwrap()))
        }));
        (release, Response::new(body))
    }
    let (first, first_response) = delayed();
    let (second, second_response) = delayed();
    let (expired, expired_response) = delayed();
    let fixture = fixture(vec![
        response(200, "<QRZDatabase><Session><Key>original-key</Key></Session></QRZDatabase>"),
        first_response, second_response, expired_response,
        response(200, "<QRZDatabase><Session><Key>second-key</Key><Error>Not found: unknown</Error></Session></QRZDatabase>"),
        response(200, "<QRZDatabase><Session><Key>second-key</Key><Error>Not found: unknown</Error></Session></QRZDatabase>"),
    ]).await?;
    let session = QrzSession::new(client()?, fixture.endpoint.clone(), material());
    let mut tasks = Vec::new();
    for count in 2..=4 {
        let session = session.clone();
        tasks.push(tokio::spawn(async move { session.lookup(&key()).await }));
        tokio::time::timeout(Duration::from_secs(3), async {
            while fixture.requests.lock().unwrap().len() < count {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await?;
    }
    let mut tasks = tasks.into_iter();
    first.send("<QRZDatabase><Session><Key>first-key</Key></Session><Callsign><call>W7TEST</call></Callsign></QRZDatabase>").unwrap();
    tasks.next().unwrap().await??;
    second.send("<QRZDatabase><Session><Key>second-key</Key></Session><Callsign><call>W7TEST</call></Callsign></QRZDatabase>").unwrap();
    tasks.next().unwrap().await??;
    expired
        .send("<QRZDatabase><Session><Error>Session Timeout</Error></Session></QRZDatabase>")
        .unwrap();
    assert!(matches!(
        tasks.next().unwrap().await??,
        ProviderLookup::NotFound
    ));
    session.lookup(&key()).await?;

    let requests = fixture.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        6,
        "stale expiry must reuse the newer key without login"
    );
    for request in requests.iter().skip(1).take(3) {
        assert!(request.uri.contains("s=original-key"));
    }
    assert!(
        requests[4].uri.contains("s=second-key"),
        "reuse the last returned key"
    );
    assert!(requests[5].uri.contains("s=second-key"));
    Ok(())
}

#[tokio::test]
async fn directory_qrz_maps_sparse_results_and_renews_session() -> anyhow::Result<()> {
    let fixture = fixture(vec![
        response(200, "<QRZDatabase><Session><Key>first-key</Key><Message>A subscription is required to obtain the complete data</Message></Session></QRZDatabase>"),
        response(200, include_str!("fixtures/qrz.xml")),
        response(200, "<QRZDatabase><Session><Error>Session Timeout</Error></Session></QRZDatabase>"),
        response(200, "<QRZDatabase><Session><Key>renewed-key</Key></Session></QRZDatabase>"),
        response(200, "<QRZDatabase><Session><Key>renewed-key</Key><Message>A subscription is required to obtain the complete data</Message></Session><Callsign><call>W7TEST</call></Callsign></QRZDatabase>"),
        response(200, "<QRZDatabase><Session><Error>Not found: unknown</Error><Key>renewed-key</Key></Session></QRZDatabase>"),
    ]).await?;
    let session = QrzSession::new(client()?, fixture.endpoint.clone(), material());
    let ProviderLookup::Found(facts) = session.lookup(&key()).await? else {
        panic!("found")
    };
    assert_eq!(facts.name.as_deref(), Some("The Boss"));
    let location = facts.location.as_ref().unwrap();
    assert_eq!(location.dxcc().as_deref(), Some("291"));
    assert!(matches!(location.position(), Some(Position::GridArea(_))));
    assert!(location.cq_zone().is_none());
    assert_eq!(
        facts.references.first().unwrap().identifier().as_deref(),
        Some("na-065")
    );
    assert!(facts.supplied_coordinates.is_some());
    let ProviderLookup::Found(sparse) = session.lookup(&key()).await? else {
        panic!("found")
    };
    assert!(sparse.name.is_none());
    assert!(sparse.location.is_none());
    assert!(matches!(
        session.lookup(&key()).await?,
        ProviderLookup::NotFound
    ));
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.len(), 6);
    assert_eq!(requests[0].method, "POST");
    assert!(!requests[0].uri.contains("password"));
    let form: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(requests[0].body.as_bytes())
            .into_owned()
            .collect();
    assert_eq!(
        form.get("password").map(String::as_str),
        Some("secret&+ sentinel")
    );
    assert!(requests[1].uri.contains("callsign=EA8%2FW7TEST"));
    assert!(requests[2].uri.contains("s=second-key"));
    assert!(requests[4].uri.contains("s=renewed-key"));
    Ok(())
}

#[tokio::test]
async fn directory_hamqth_distinguishes_expiry_rejection_and_rate_limit() -> anyhow::Result<()> {
    let limited = Response::builder()
        .status(429)
        .header("retry-after", "7")
        .body(Body::from("{}"))?;
    let fixture = fixture(vec![
        response(200, "{\"token\":\"first-token\"}"),
        response(200, include_str!("fixtures/hamqth.json")),
        response(401, "{}"),
        response(200, "{\"token\":\"renewed-token\"}"),
        response(200, "{\"callsign\":\"OK2TEST\"}"),
        limited,
        response(401, "{}"),
        response(401, "{}"),
    ])
    .await?;
    let session = HamQthSession::new(client()?, fixture.endpoint.clone(), material());
    let ProviderLookup::Found(facts) = session.lookup(&key()).await? else {
        panic!("found")
    };
    assert_eq!(facts.name.as_deref(), Some("Fixture operator"));
    let location = facts.location.as_ref().unwrap();
    assert_eq!(location.dxcc().as_deref(), Some("503"));
    assert!(
        matches!(location.position(), Some(Position::GridArea(grid)) if grid.length().get() == 6)
    );
    assert!(location.cq_zone().is_none());
    assert!(facts.supplied_coordinates.is_some());
    let ProviderLookup::Found(sparse) = session.lookup(&key()).await? else {
        panic!("found")
    };
    assert!(sparse.name.is_none());
    assert!(
        matches!(session.lookup(&key()).await, Err(ProviderError::RateLimited { retry_after: Some(delay) }) if delay == Duration::from_secs(7))
    );
    assert!(matches!(
        session.lookup(&key()).await,
        Err(ProviderError::Authentication)
    ));
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.len(), 8, "a rejected renewal must not loop");
    let login: serde_json::Value = serde_json::from_str(&requests[0].body)?;
    assert_eq!(login["password"], "secret&+ sentinel");
    assert_eq!(requests[0].uri, "/login");
    assert!(requests[1].uri.contains("/callbook/EA8%2FW7TEST"));
    assert!(requests[1].uri.contains("app=Clacks"));
    assert_eq!(
        requests[1].authorization.as_deref(),
        Some("Bearer first-token")
    );
    assert_eq!(
        requests[4].authorization.as_deref(),
        Some("Bearer renewed-token")
    );
    Ok(())
}

#[tokio::test]
async fn directory_transport_rejects_redirects_and_unusable_responses_without_auth_pause()
-> anyhow::Result<()> {
    let target = fixture(vec![]).await?;
    let redirect = Response::builder()
        .status(307)
        .header("location", target.endpoint.as_str())
        .body(Body::empty())?;
    let original = fixture(vec![redirect]).await?;
    let qrz = QrzSession::new(client()?, original.endpoint.clone(), material());
    assert!(matches!(
        qrz.lookup(&key()).await,
        Err(ProviderError::Unavailable {
            reason: SafeProviderFailure::Refusal,
            ..
        })
    ));
    assert!(
        target.requests.lock().unwrap().is_empty(),
        "credentials must not follow redirects"
    );

    let repeated_expiry = fixture(vec![
        response(200, "{\"token\":\"one\"}"),
        response(401, "{}"),
        response(200, "{\"token\":\"two\"}"),
        response(401, "{}"),
    ])
    .await?;
    let hamqth = HamQthSession::new(client()?, repeated_expiry.endpoint.clone(), material());
    assert!(
        matches!(
            hamqth.lookup(&key()).await,
            Err(ProviderError::Unavailable {
                reason: SafeProviderFailure::InvalidResponse,
                ..
            })
        ),
        "an expired bearer token does not prove that login credentials were rejected"
    );
    assert_eq!(repeated_expiry.requests.lock().unwrap().len(), 4);

    let oversized = Response::builder()
        .status(200)
        .body(Body::from(vec![b'x'; super::MAX_BODY + 1]))?;
    let invalid = fixture(vec![response(200,"<QRZDatabase><Session><Key>one</Key></Session></QRZDatabase>"), oversized, response(200,"<!DOCTYPE QRZDatabase [<!ENTITY secret 'fixture'>]><QRZDatabase><Session><Key>one</Key></Session><Callsign><call>W7TEST</call><name>&secret;</name></Callsign></QRZDatabase>")]).await?;
    let qrz = QrzSession::new(client()?, invalid.endpoint.clone(), material());
    assert!(matches!(
        qrz.lookup(&key()).await,
        Err(ProviderError::Unavailable {
            reason: SafeProviderFailure::InvalidResponse,
            ..
        })
    ));
    let failure = qrz.lookup(&key()).await.unwrap_err();
    assert!(matches!(
        failure,
        ProviderError::Unavailable {
            reason: SafeProviderFailure::Parse,
            ..
        }
    ));
    assert!(!format!("{failure:?}").contains("secret"));
    Ok(())
}

#[tokio::test]
async fn directory_explicit_auth_retry_reauthenticates_instead_of_reusing_rejected_key()
-> anyhow::Result<()> {
    let fixture = fixture(vec![response(200, "<QRZDatabase><Session><Key>old-key</Key></Session></QRZDatabase>"), response(401, ""), response(200, "<QRZDatabase><Session><Key>retried-key</Key></Session></QRZDatabase>"), response(200, "<QRZDatabase><Session><Key>retried-key</Key><Error>Not found: unknown</Error></Session></QRZDatabase>")]).await?;
    let mut session = super::ProviderSession::Qrz(QrzSession::new(
        client()?,
        fixture.endpoint.clone(),
        material(),
    ));
    assert!(matches!(
        session.lookup(&key()).await,
        Err(ProviderError::Authentication)
    ));
    session.reset_authentication();
    assert!(matches!(
        session.lookup(&key()).await?,
        ProviderLookup::NotFound
    ));
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].method, "POST");
    assert!(requests[3].uri.contains("s=retried-key"));
    Ok(())
}
