//! The production account page and server-function boundary expose only owned UI data.

#![cfg(feature = "ssr")]

use crate::support::provider;

use anyhow::Context;
use huskarl_core::dpop::{AuthorizationServerDPoP, DPoP, ResourceServerDPoP};
use leptos::server_fn::ServerFn;
use reqwest::cookie::CookieStore;

#[cfg(feature = "ssr")]
#[tokio::test]
async fn account_page_and_direct_server_function_require_the_same_login() -> anyhow::Result<()> {
    let registrations = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let provider = provider::start(Some(registrations.clone())).await?;
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

    let router = clacks_server::http::router(state, pages)?;
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let landing = client.get(&base).send().await?;
    assert_eq!(landing.status(), reqwest::StatusCode::OK);
    let landing = landing.text().await?;
    assert!(landing.contains("href=\"/download\""));
    assert!(landing.contains("href=\"/sign-in\""));

    let download = client.get(format!("{base}/download")).send().await?;
    assert_eq!(download.status(), reqwest::StatusCode::OK);
    assert!(
        download
            .text()
            .await?
            .contains("https://github.com/jhelwig/clacks-log/releases")
    );

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

    let key = huskarl_crypto_native::asymmetric::signer::PrivateKey::generate(
        huskarl_crypto_native::asymmetric::signer::GenerateAlgorithm::Es256,
        None,
    )?;
    let thumbprint = key.as_private_jwk().thumbprint();
    let dpop = DPoP::builder().signer(key).build();
    let begin = client
        .get(format!("{base}/oauth/authorize"))
        .query(&[
            ("client_id", "clacks-desktop"),
            ("response_type", "code"),
            ("redirect_uri", "http://127.0.0.1:49125/oauth/callback"),
            ("scope", "sync"),
            ("state", "retained-desktop-request"),
            ("client_label", "Windows desktop"),
            ("code_challenge_method", "S256"),
            (
                "code_challenge",
                "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            ),
            ("dpop_jkt", thumbprint.as_str()),
        ])
        .send()
        .await?;
    assert!(begin.headers().get("location").is_some_and(|value| {
        value
            .to_str()
            .is_ok_and(|value| value.starts_with("/sign-in?enrollment="))
    }));

    let start = client
        .post(format!("{base}{}", clacks_web::actions::StartSignIn::PATH))
        .header("Origin", &base)
        .header("Accept", "text/html")
        .form(&[
            ("provider", "test"),
            ("enrollment", "retained-desktop-request"),
        ])
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
    assert_eq!(
        signed_in.headers().get("location").unwrap(),
        "/account/clients/approve?request=retained-desktop-request"
    );
    let approval = client
        .get(format!(
            "{base}/account/clients/approve?request=retained-desktop-request"
        ))
        .send()
        .await?
        .text()
        .await?;
    assert!(approval.contains("Windows desktop"));
    let approve_path = clacks_web::actions::clients::DecideApproval::PATH;
    assert_eq!(
        client
            .post(format!("{base}{approve_path}"))
            .form(&[
                ("request", "retained-desktop-request"),
                ("decision", "approve")
            ])
            .send()
            .await?
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    let approved = client
        .post(format!("{base}{approve_path}"))
        .header("Origin", &base)
        .header("Accept", "text/html")
        .form(&[
            ("request", "retained-desktop-request"),
            ("decision", "approve"),
        ])
        .send()
        .await?;
    let callback = url::Url::parse(
        approved
            .headers()
            .get("location")
            .context("approval omitted redirect")?
            .to_str()?,
    )?;
    let code = callback
        .query_pairs()
        .find(|(key, _)| key == "code")
        .context("approval omitted code")?
        .1
        .into_owned();
    let token_target = format!("{base}/oauth/token").parse()?;
    let proof = dpop
        .proof(&axum::http::Method::POST, &token_target, Some(&thumbprint))
        .await?
        .context("missing proof")?;
    let issued: serde_json::Value = client
        .post(format!("{base}/oauth/token"))
        .header("dpop", proof.expose_secret())
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", "clacks-desktop"),
            ("code", code.as_str()),
            ("redirect_uri", "http://127.0.0.1:49125/oauth/callback"),
            (
                "code_verifier",
                "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            ),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let token = issued
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .context("missing issued access")?;
    let access = huskarl_core::secrets::SecretString::from(token);
    let identity_target = format!("{base}/api/client").parse()?;
    let proof = dpop
        .to_resource_server_dpop()
        .proof(
            &axum::http::Method::GET,
            &identity_target,
            &access,
            &thumbprint,
        )
        .await?
        .context("missing resource proof")?;
    let identity: serde_json::Value = client
        .get(format!("{base}/api/client"))
        .header("authorization", format!("DPoP {token}"))
        .header("dpop", proof.expose_secret())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    // Recovery hints cannot authorize another account, even in a live browser session.
    let wrong_account = ulid::Ulid::generate().to_string();
    let actor = identity
        .get("actor")
        .and_then(serde_json::Value::as_str)
        .context("missing actor")?;
    let recovery = client
        .get(format!("{base}/oauth/authorize"))
        .query(&[
            ("client_id", "clacks-desktop"),
            ("response_type", "code"),
            ("redirect_uri", "http://127.0.0.1:49125/oauth/callback"),
            ("scope", "sync"),
            ("state", "wrong-account-recovery"),
            ("client_label", "Wrong account"),
            ("code_challenge_method", "S256"),
            (
                "code_challenge",
                "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            ),
            ("dpop_jkt", thumbprint.as_str()),
            ("clacks_account", &wrong_account),
            ("clacks_actor", actor),
        ])
        .send()
        .await?;
    assert!(recovery.status().is_redirection());
    let refused = client
        .post(format!("{base}{approve_path}"))
        .header("origin", &base)
        .header("accept", "application/json")
        .form(&[
            ("request", "wrong-account-recovery"),
            ("decision", "approve"),
        ])
        .send()
        .await?;
    assert!(refused.headers().get("location").is_none());
    assert!(
        refused
            .text()
            .await?
            .contains("Sign in to the account associated with this desktop workspace")
    );

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
    let account_value: serde_json::Value = serde_json::from_str(&account)?;
    assert_eq!(identity.get("account"), account_value.get("id"));
    let actor = identity
        .get("actor")
        .and_then(serde_json::Value::as_str)
        .context("missing actor")?;
    let management = client
        .get(format!("{base}/account/clients"))
        .send()
        .await?
        .text()
        .await?;
    assert!(management.contains("Windows desktop"));
    assert!(management.contains(actor));
    let revoked = client
        .post(format!(
            "{base}{}",
            clacks_web::actions::clients::RevokeClient::PATH
        ))
        .header("Origin", &base)
        .form(&[("actor", actor)])
        .send()
        .await?;
    assert!(revoked.status().is_success());
    let proof = dpop
        .to_resource_server_dpop()
        .proof(
            &axum::http::Method::GET,
            &identity_target,
            &access,
            &thumbprint,
        )
        .await?
        .context("missing fresh resource proof")?;
    assert_eq!(
        client
            .get(format!("{base}/api/client"))
            .header("authorization", format!("DPoP {token}"))
            .header("dpop", proof.expose_secret())
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
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
    assert_eq!(registrations.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(jar.cookies(&url::Url::parse(&base)?).is_some());
    task.abort();
    Ok(())
}
