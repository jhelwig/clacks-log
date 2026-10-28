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
        axum::routing::get(|_: clacks_server::auth::principal::Principal| async {
            axum::http::StatusCode::OK
        }),
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
    let logout = client
        .post(format!("{base}/auth/logout"))
        .header("Origin", &base)
        .header("Cookie", &cookie)
        .send()
        .await?;
    assert_eq!(logout.status(), reqwest::StatusCode::SEE_OTHER);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let other = format!("http://{}", listener.local_addr()?);
    let second_state = ServerState::connect(config).await?;
    let pages = axum::Router::new().route(
        "/protected",
        axum::routing::get(|_: clacks_server::auth::principal::Principal| async {
            axum::http::StatusCode::OK
        }),
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
