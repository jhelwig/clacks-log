//! Registered metadata survives setup and governs production sign-in.

use crate::support;
use anyhow::Context;
use leptos::server_fn::ServerFn;

use clacks_server::{
    config::{ProviderConfig, RegistrationMode, ServerConfig},
    maintenance::registration,
    runtime::ServerState,
};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn registration_reuses_saved_client_and_preserves_it_on_mismatch() -> anyhow::Result<()> {
    let registrations = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let provider = support::provider::start(Some(registrations.clone())).await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("registration.json");
    let config = ProviderConfig {
        issuer: provider.issuer.clone(),
        label: "Test".into(),
        registration: RegistrationMode::Dynamic {
            registration_file: path.clone(),
            initial_access_token_file: None,
        },
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let callback = url::Url::parse(&format!("{base}/auth/callback/test"))?;
    registration::run(&config, &callback).await?;
    let original = std::fs::read(&path)?;
    let registered: serde_json::Value = serde_json::from_slice(&original)?;
    assert_eq!(registered["response"]["client_name"], "Clacks Log");
    registration::run(&config, &callback).await?;
    assert_eq!(registrations.load(Ordering::SeqCst), 1);
    assert!(
        registration::run(&config, &url::Url::parse("https://other.example/callback")?)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&path)?, original);

    let database = directory.path().join("runtime-url");
    let cookie_key = directory.path().join("cookie-key");
    std::fs::write(
        &database,
        std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?,
    )?;
    std::fs::write(&cookie_key, [42; 64])?;
    let wrapping_file = directory.path().join("wrapping-key");
    std::fs::write(&wrapping_file, [19; 32])?;
    let runtime: ServerConfig = serde_json::from_value(serde_json::json!({
        "public_url": base, "allow_http": true,
        "database_url_file": database, "cookie_key_file": cookie_key,
        "wrapping_keys":{"active":"test","files":{"test":wrapping_file}},
        "providers": {"test": {"issuer": provider.issuer, "label": "Fixture",
        "mode": "dynamic", "registration_file": path}}
    }))?;
    let state = ServerState::connect(runtime.clone()).await?;
    let options = leptos::config::LeptosOptions::builder()
        .output_name("clacks-web")
        .build();
    let pages = clacks_web::routes(state.clone(), options);
    let router = clacks_server::http::router(state, pages)?;
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let start = client
        .post(format!("{base}{}", clacks_web::actions::StartSignIn::PATH))
        .header("Accept", "text/html")
        .form(&[("provider", "test")])
        .header("Origin", &base)
        .send()
        .await?;
    let authorization = url::Url::parse(start.headers().get("location").unwrap().to_str()?)?;
    let returned = provider.authorize(&authorization, "registered-operator")?;
    let completed = client
        .get(format!("{base}/auth/callback/test"))
        .query(&[("code", returned.code), ("state", returned.state)])
        .send()
        .await?;
    assert_eq!(completed.status(), reqwest::StatusCode::SEE_OTHER);
    let admitted: clacks_web::actions::Account = client
        .post(format!("{base}{}", clacks_web::actions::LoadAccount::PATH))
        .header("Origin", &base)
        .body("")
        .send()
        .await?
        .json()
        .await?;
    assert!(
        admitted
            .identities
            .iter()
            .any(|identity| identity.issuer == provider.issuer.as_str()
                && identity.subject == "registered-operator")
    );
    task.abort();

    let mut unsupported: serde_json::Value = serde_json::from_slice(&original)?;
    unsupported["response"]["token_endpoint_auth_method"] = serde_json::json!("private_key_jwt");
    let unsupported = serde_json::to_vec(&unsupported)?;
    std::fs::write(&path, &unsupported)?;
    let refusal = ServerState::connect(runtime)
        .await
        .err()
        .context("unsupported registration must prevent startup")?;
    assert!(refusal.to_string().contains("token_endpoint_auth_method"));
    assert_eq!(std::fs::read(&path)?, unsupported);

    let mut expired: serde_json::Value = serde_json::from_slice(&original)?;
    expired["response"]["client_secret_expires_at"] = serde_json::json!(1);
    std::fs::write(&path, serde_json::to_vec(&expired)?)?;
    let refusal = registration::run(&config, &callback).await.unwrap_err();
    assert!(refusal.to_string().contains("expired"));
    assert_eq!(registrations.load(Ordering::SeqCst), 1);
    Ok(())
}
