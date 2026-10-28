//! Account workflows through the production HTTP and persistence boundaries.

use crate::support;

use clacks_server::{config::ServerConfig, runtime::ServerState};
use openidconnect::reqwest;
use reqwest::cookie::CookieStore;

#[tokio::test]
async fn logout_rejects_stale_cookie_on_another_instance() -> anyhow::Result<()> {
    let logs = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .without_time()
        .with_env_filter("clacks_server=trace,axum_session=off")
        .with_writer(logs.reopen()?)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    let provider = support::provider::start().await?;
    let directory = tempfile::tempdir()?;
    let url_file = directory.path().join("database-url");
    std::fs::write(
        &url_file,
        std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?,
    )?;
    let cookie_file = directory.path().join("cookie-key");
    std::fs::write(&cookie_file, [42; 64])?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let config: ServerConfig = serde_json::from_value(serde_json::json!({
        "public_url": base, "allow_http": true, "database_url_file": url_file,
        "cookie_key_file": cookie_file, "providers": {"test": {
            "issuer": provider.issuer, "label": "Fixture", "mode": "preconfigured",
            "client_id": "registered-clacks"
        }}
    }))?;
    let state = ServerState::connect(config.clone()).await?;
    let pages = axum::Router::new().route(
        "/protected",
        axum::routing::get(|principal: clacks_server::auth::principal::Principal| async move { axum::Json(serde_json::json!({"account_id": principal.account_id.to_string(), "identity_id": principal.identity_id.to_string()})) }),
    );
    let router = clacks_server::http::router(state.clone(), pages);
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let start = client
        .post(format!("{base}/auth/start/test"))
        .header("Origin", &base)
        .send()
        .await?;
    assert_eq!(start.status(), reqwest::StatusCode::SEE_OTHER);
    let authorization = url::Url::parse(start.headers().get("location").unwrap().to_str()?)?;
    let callback = provider.authorize(&authorization, "first-operator")?;
    let invalid = client
        .get(format!("{base}/auth/callback/test"))
        .query(&[
            ("code", &callback.code),
            ("state", &"wrong-state".to_owned()),
        ])
        .send()
        .await?;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    let signed_in = client
        .get(format!("{base}/auth/callback/test"))
        .query(&[("code", &callback.code), ("state", &callback.state)])
        .send()
        .await?;
    assert_eq!(signed_in.status(), reqwest::StatusCode::SEE_OTHER);
    let cookie = jar
        .cookies(&url::Url::parse(&base)?)
        .unwrap()
        .to_str()?
        .to_owned();
    assert_eq!(
        client
            .get(format!("{base}/protected"))
            .header("Cookie", &cookie)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::OK
    );
    let before: serde_json::Value = client
        .get(format!("{base}/protected"))
        .send()
        .await?
        .json()
        .await?;
    let principal = clacks_server::auth::principal::Principal {
        account_id: before["account_id"].as_str().unwrap().parse()?,
        identity_id: before["identity_id"].as_str().unwrap().parse()?,
    };
    let authorizer = principal.identity_id.to_string();
    let link = client
        .post(format!("{base}/account/identities/link"))
        .header("Origin", &base)
        .form(&[("provider", "test"), ("authorizing_identity", &authorizer)])
        .send()
        .await?;
    let authorization = url::Url::parse(link.headers().get("location").unwrap().to_str()?)?;
    assert!(
        authorization
            .query_pairs()
            .any(|(key, value)| key == "max_age" && value == "300")
    );
    let old = provider.authorize_at(
        &authorization,
        "first-operator",
        chrono::Utc::now() - chrono::Duration::minutes(6),
    )?;
    let stale = client
        .get(format!("{base}/auth/callback/test"))
        .query(&[("code", old.code), ("state", old.state)])
        .send()
        .await?;
    assert_eq!(stale.status(), reqwest::StatusCode::BAD_REQUEST);
    let wrong = client
        .post(format!("{base}/account/identities/link"))
        .header("Origin", &base)
        .form(&[("provider", "test"), ("authorizing_identity", &authorizer)])
        .send()
        .await?;
    let wrong = return_from_provider(&client, &provider, wrong, "wrong-operator").await?;
    assert_eq!(wrong.status(), reqwest::StatusCode::CONFLICT);
    let link = client
        .post(format!("{base}/account/identities/link"))
        .header("Origin", &base)
        .form(&[("provider", "test"), ("authorizing_identity", &authorizer)])
        .send()
        .await?;
    let next = return_from_provider(&client, &provider, link, "first-operator").await?;
    assert_eq!(next.status(), reqwest::StatusCode::SEE_OTHER);
    let linked = return_from_provider(&client, &provider, next, "second-operator").await?;
    assert_eq!(linked.status(), reqwest::StatusCode::SEE_OTHER);
    let after: serde_json::Value = client
        .get(format!("{base}/protected"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(after, before);
    assert_eq!(state.store.identities(&principal).await?.len(), 2);
    let pending = client
        .post(format!("{base}/account/identities/link"))
        .header("Origin", &base)
        .form(&[("provider", "test"), ("authorizing_identity", &authorizer)])
        .send()
        .await?;
    let pending = return_from_provider(&client, &provider, pending, "first-operator").await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let other = format!("http://{}", listener.local_addr()?);
    let second_state = ServerState::connect(config).await?;
    let pages = axum::Router::new().route(
        "/protected",
        axum::routing::get(|principal: clacks_server::auth::principal::Principal| async move { axum::Json(serde_json::json!({"account_id": principal.account_id.to_string(), "identity_id": principal.identity_id.to_string()})) }),
    );
    let second = clacks_server::http::router(second_state, pages);
    let second_server = tokio::spawn(async move {
        axum::serve(listener, second).await.unwrap();
    });
    assert_eq!(
        client
            .get(format!("{other}/protected"))
            .header("Cookie", &cookie)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::OK
    );
    let logout = client
        .post(format!("{base}/auth/logout"))
        .header("Origin", &base)
        .header("Cookie", &cookie)
        .send()
        .await?;
    assert_eq!(logout.status(), reqwest::StatusCode::SEE_OTHER);
    let cancelled = return_from_provider(&client, &provider, pending, "never-linked").await?;
    assert_ne!(cancelled.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(state.store.identities(&principal).await?.len(), 2);
    assert_eq!(
        client
            .get(format!("{other}/protected"))
            .header("Cookie", &cookie)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let captured = std::fs::read_to_string(logs.path())?;
    assert!(captured.contains("Browser login established"));
    assert!(captured.contains("Browser login invalidated"));
    assert!(!captured.contains(&callback.code));
    assert!(!captured.contains(&callback.state));
    assert!(!captured.contains("first-operator"));
    server.abort();
    second_server.abort();
    Ok(())
}

/// Simulate provider approval while exercising the application's real callback.
async fn return_from_provider(
    client: &reqwest::Client,
    provider: &support::provider::Provider,
    redirect: reqwest::Response,
    subject: &str,
) -> anyhow::Result<reqwest::Response> {
    let authorization = url::Url::parse(redirect.headers().get("location").unwrap().to_str()?)?;
    let callback_url = authorization
        .query_pairs()
        .find(|(key, _)| key == "redirect_uri")
        .unwrap()
        .1
        .into_owned();
    let callback = provider.authorize(&authorization, subject)?;
    Ok(client
        .get(callback_url)
        .query(&[("code", callback.code), ("state", callback.state)])
        .send()
        .await?)
}
