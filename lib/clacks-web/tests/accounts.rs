//! Account workflows through the production HTTP and persistence boundaries.

use crate::support;

use leptos::server_fn::ServerFn;

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
    let options = leptos::config::LeptosOptions::builder()
        .output_name("clacks-web")
        .build();
    let pages = clacks_web::routes(state.clone(), options.clone());
    let router = clacks_server::http::router(state.clone(), pages);
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let unknown_provider = "untrusted-private-provider-marker";
    let unknown = client
        .get(format!("{base}/auth/callback/{unknown_provider}"))
        .query(&[("state", "missing-context")])
        .send()
        .await?;
    assert!(
        unknown
            .headers()
            .get("location")
            .unwrap()
            .to_str()?
            .starts_with("/sign-in?error=")
    );
    let start = client
        .post(format!("{base}{}", clacks_web::actions::StartSignIn::PATH))
        .form(&[("provider", "test")])
        .header("Origin", &base)
        .header("Accept", "text/html")
        .send()
        .await?;
    let authorization = url::Url::parse(start.headers().get("location").unwrap().to_str()?)?;
    let callback = provider.authorize(&authorization, "first-operator")?;
    let browser_failure = client
        .get(format!("{base}/auth/callback/test"))
        .query(&[
            ("code", &callback.code),
            ("state", &"wrong-state".to_owned()),
        ])
        .header("Accept", "text/html")
        .send()
        .await?;
    let restart = browser_failure
        .headers()
        .get("location")
        .unwrap()
        .to_str()?;
    assert!(restart.starts_with("/sign-in?error="));
    assert!(!restart.contains(&callback.code));
    assert!(!restart.contains(&callback.state));
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
            .post(format!("{base}{}", clacks_web::actions::LoadAccount::PATH))
            .header("Origin", &base)
            .body("")
            .header("Cookie", &cookie)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::OK
    );
    let before: clacks_web::actions::Account = client
        .post(format!("{base}{}", clacks_web::actions::LoadAccount::PATH))
        .header("Origin", &base)
        .body("")
        .send()
        .await?
        .json()
        .await?;
    let principal = clacks_server::auth::principal::Principal {
        account_id: before.id.parse()?,
        identity_id: before.current_identity.parse()?,
    };
    let authorizer = principal.identity_id.to_string();
    let link = client
        .post(format!("{base}{}", clacks_web::actions::StartLink::PATH))
        .header("Origin", &base)
        .header("Accept", "text/html")
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
    assert!(
        stale
            .headers()
            .get("location")
            .unwrap()
            .to_str()?
            .starts_with("/sign-in?error=")
    );
    let wrong = client
        .post(format!("{base}{}", clacks_web::actions::StartLink::PATH))
        .header("Origin", &base)
        .header("Accept", "text/html")
        .form(&[("provider", "test"), ("authorizing_identity", &authorizer)])
        .send()
        .await?;
    let wrong = return_from_provider(&client, &provider, wrong, "wrong-operator").await?;
    assert!(
        wrong
            .headers()
            .get("location")
            .unwrap()
            .to_str()?
            .starts_with("/sign-in?error=")
    );
    let link = client
        .post(format!("{base}{}", clacks_web::actions::StartLink::PATH))
        .header("Origin", &base)
        .header("Accept", "text/html")
        .form(&[("provider", "test"), ("authorizing_identity", &authorizer)])
        .send()
        .await?;
    let next = return_from_provider(&client, &provider, link, "first-operator").await?;
    assert_eq!(next.status(), reqwest::StatusCode::SEE_OTHER);
    let linked = return_from_provider(&client, &provider, next, "second-operator").await?;
    assert_eq!(linked.status(), reqwest::StatusCode::SEE_OTHER);
    let after: clacks_web::actions::Account = client
        .post(format!("{base}{}", clacks_web::actions::LoadAccount::PATH))
        .header("Origin", &base)
        .body("")
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(after.id, before.id);
    assert_eq!(after.current_identity, before.current_identity);
    assert_eq!(state.store.identities(&principal).await?.len(), 2);
    let pending = client
        .post(format!("{base}{}", clacks_web::actions::StartLink::PATH))
        .header("Origin", &base)
        .header("Accept", "text/html")
        .form(&[("provider", "test"), ("authorizing_identity", &authorizer)])
        .send()
        .await?;
    let pending = return_from_provider(&client, &provider, pending, "first-operator").await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let other = format!("http://{}", listener.local_addr()?);
    let second_state = ServerState::connect(config).await?;
    let pages = clacks_web::routes(second_state.clone(), options);
    let second = clacks_server::http::router(second_state, pages);
    let second_server = tokio::spawn(async move {
        axum::serve(listener, second).await.unwrap();
    });
    assert_eq!(
        client
            .post(format!("{other}{}", clacks_web::actions::LoadAccount::PATH))
            .header("Origin", &base)
            .body("")
            .header("Cookie", &cookie)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::OK
    );
    let logout = client
        .post(format!("{base}{}", clacks_web::actions::SignOut::PATH))
        .body("")
        .header("Origin", &base)
        .header("Accept", "text/html")
        .header("Cookie", &cookie)
        .send()
        .await?;
    assert_eq!(logout.headers().get("location").unwrap(), "/sign-in");
    let cancelled = return_from_provider(&client, &provider, pending, "never-linked").await?;
    assert_eq!(
        cancelled.headers().get("location").unwrap(),
        "/sign-in?error=Authorization+failed+or+expired.+Restart+the+action."
    );
    assert_eq!(state.store.identities(&principal).await?.len(), 2);
    let refused = client
        .post(format!("{other}{}", clacks_web::actions::LoadAccount::PATH))
        .header("Origin", &base)
        .header("Cookie", &cookie)
        .body("")
        .send()
        .await?;
    assert_eq!(refused.headers().get("location").unwrap(), "/sign-in");
    assert!(!refused.text().await?.contains("first-operator"));
    let captured = std::fs::read_to_string(logs.path())?;
    assert!(captured.contains("Browser login established"));
    assert!(captured.contains("Browser login invalidated"));
    assert!(!captured.contains(&callback.code));
    assert!(!captured.contains(&callback.state));
    assert!(!captured.contains(unknown_provider));
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
