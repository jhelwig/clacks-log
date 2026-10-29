//! The production account page and server-function boundary expose only owned UI data.

#![cfg(feature = "ssr")]

use crate::support::provider;

use leptos::server_fn::ServerFn;
use reqwest::cookie::CookieStore;

#[cfg(feature = "ssr")]
#[tokio::test]
async fn account_page_and_direct_server_function_require_the_same_login() -> anyhow::Result<()> {
    let provider = provider::start().await?;
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("runtime-url");
    let key = directory.path().join("cookie-key");
    std::fs::write(
        &database,
        std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?,
    )?;
    std::fs::write(&key, [42; 64])?;
    let wrapping_file = directory.path().join("wrapping-key");
    std::fs::write(&wrapping_file, [19; 32])?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let config = serde_json::from_value(serde_json::json!({
        "public_url": base, "allow_http": true, "database_url_file": database,
        "cookie_key_file": key, "wrapping_keys":{"active":"test","files":{"test":wrapping_file}}, "providers": {"test": {"issuer": provider.issuer,
        "label": "OIDC Provider Name", "mode": "preconfigured", "client_id": "registered-clacks"}}
    }))?;
    let state = clacks_server::runtime::ServerState::connect(config).await?;
    let assets = directory.path().join("pkg");
    std::fs::create_dir(&assets)?;
    std::fs::write(
        assets.join("clacks-web.css"),
        include_str!("../style/account.css"),
    )?;
    let options = leptos::config::LeptosOptions::builder()
        .output_name("clacks-web")
        .site_root(directory.path().to_string_lossy().into_owned())
        .build();
    let pages = clacks_web::routes(state.clone(), options);

    let router = clacks_server::http::router(state, pages);
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let sign_in = client
        .get(format!("{base}/sign-in"))
        .send()
        .await?
        .text()
        .await?;
    assert!(sign_in.contains("OIDC Provider Name"));
    let stylesheet = client
        .get(format!("{base}/pkg/clacks-web.css"))
        .send()
        .await?;
    assert!(stylesheet.status().is_success());
    assert!(stylesheet.text().await?.contains("--section"));

    let start = client
        .post(format!("{base}{}", clacks_web::actions::StartSignIn::PATH))
        .header("Origin", &base)
        .header("Accept", "text/html")
        .form(&[("provider", "test")])
        .send()
        .await?;
    let authorization = url::Url::parse(start.headers().get("location").unwrap().to_str()?)?;
    let returned = provider.authorize(&authorization, "web-account-operator")?;
    let signed_in = client
        .get(format!("{base}/auth/callback/test"))
        .query(&[("code", &returned.code), ("state", &returned.state)])
        .send()
        .await?;
    assert!(signed_in.status().is_redirection());
    let page = client
        .get(format!("{base}/account/identities"))
        .send()
        .await?
        .text()
        .await?;
    assert!(page.contains("web-account-operator"));
    assert!(!page.contains("postgres://"));
    assert!(!page.contains("cookie_key_file"));
    assert!(!page.contains(&returned.code));
    assert!(!page.contains(&returned.state));
    let account = client
        .post(format!("{base}{}", clacks_web::actions::LoadAccount::PATH))
        .header("Origin", &base)
        .body("")
        .send()
        .await?
        .text()
        .await?;
    assert!(account.contains("web-account-operator"));
    let outsider = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let anonymous = outsider
        .post(format!("{base}{}", clacks_web::actions::LoadAccount::PATH))
        .header("Origin", &base)
        .body("")
        .send()
        .await?;
    assert_eq!(anonymous.headers().get("location").unwrap(), "/sign-in");
    assert!(!anonymous.text().await?.contains("web-account-operator"));
    let removal = outsider
        .post(format!("{base}{}", clacks_web::actions::StartUnlink::PATH))
        .header("Origin", &base)
        .form(&[
            ("target", "untrusted-id"),
            ("authorizing_identity", "untrusted-id"),
        ])
        .send()
        .await?;
    assert_eq!(removal.headers().get("location").unwrap(), "/sign-in");
    assert_eq!(
        provider
            .registrations
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(jar.cookies(&url::Url::parse(&base)?).is_some());
    task.abort();
    Ok(())
}
