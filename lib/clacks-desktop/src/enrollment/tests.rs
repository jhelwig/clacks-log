//! Real native persistence and issuer interoperability; no synthetic authorization service.

#[path = "../../../clacks-web/tests/support/provider.rs"]
mod provider;

use super::{EnrollmentRequest, EnrollmentStatus, credentials, flow};
use anyhow::Context;
use leptos::server_fn::ServerFn;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Delete only the unique native entry created by this scenario, including on failure.
struct NativeEntry(String);

impl Drop for NativeEntry {
    fn drop(&mut self) {
        let _ = credentials::remove(&self.0);
    }
}

#[tokio::test]
async fn saved_native_credential_resumes_the_actual_client() -> anyhow::Result<()> {
    let provider = provider::start().await?;
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("database-url");
    std::fs::write(&database, std::env::var("CLACKS_TEST_DATABASE_URL")?)?;
    let cookie = directory.path().join("cookie-key");
    std::fs::write(&cookie, [45; 64])?;
    let wrapping = directory.path().join("wrapping-key");
    std::fs::write(&wrapping, [17; 32])?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let config = serde_json::from_value(serde_json::json!({"public_url":origin,"allow_http":true,
        "database_url_file":database,"cookie_key_file":cookie,"wrapping_keys":{"active":"test","files":{"test":wrapping}},
        "providers":{"test":{"issuer":provider.issuer,"label":"Test identity provider","mode":"preconfigured","client_id":"registered-clacks"}}}))?;
    let state = clacks_server::runtime::ServerState::connect(config).await?;
    let options = leptos::config::LeptosOptions::builder()
        .output_name("clacks-web")
        .build();
    let router = clacks_server::http::router(state.clone(), clacks_web::routes(state, options));
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let reference = credentials::create(&origin)?;
    let _entry = NativeEntry(reference.clone());
    let request = EnrollmentRequest {
        server: origin.clone(),
        label: "Native persistence desktop".into(),
        credential_reference: reference.clone(),
        expected: None,
    };
    let (status, mut progress) = watch::channel(EnrollmentStatus::Idle);
    let authorizing = tokio::spawn(flow::authorize(request, status, CancellationToken::new()));
    let authorization = loop {
        if progress.changed().await.is_err() {
            match authorizing.await? {
                Err(error) => return Err(error),
                Ok(_) => anyhow::bail!("Authorization ended before publishing its browser request"),
            }
        }
        if let EnrollmentStatus::Authorizing { authorization_url } = progress.borrow().clone() {
            break authorization_url;
        }
    };
    let correlation = url::Url::parse(&authorization)?
        .query_pairs()
        .find(|(key, _)| key == "state")
        .context("missing request correlation")?
        .1
        .into_owned();
    let browser = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let start = browser.get(&authorization).send().await?;
    assert!(start.status().is_redirection());
    let login = browser
        .post(format!(
            "{origin}{}",
            clacks_web::actions::StartSignIn::PATH
        ))
        .header("origin", &origin)
        .header("accept", "text/html")
        .form(&[("provider", "test"), ("enrollment", correlation.as_str())])
        .send()
        .await?;
    let provider_request = url::Url::parse(
        login
            .headers()
            .get("location")
            .context("missing provider redirect")?
            .to_str()?,
    )?;
    let returned = provider.authorize(&provider_request, &ulid::Ulid::generate().to_string())?;
    browser
        .get(format!("{origin}/auth/callback/test"))
        .query(&[("state", returned.state), ("code", returned.code)])
        .send()
        .await?
        .error_for_status()?;
    let approved = browser
        .post(format!(
            "{origin}{}",
            clacks_web::actions::clients::DecideApproval::PATH
        ))
        .header("origin", &origin)
        .header("accept", "text/html")
        .form(&[("request", correlation.as_str()), ("decision", "approve")])
        .send()
        .await?;
    browser
        .get(
            approved
                .headers()
                .get("location")
                .context("missing native callback")?
                .to_str()?,
        )
        .send()
        .await?
        .error_for_status()?;
    let first = authorizing.await??;
    let before = credentials::load(&reference)?;
    let refresh = before
        .refresh
        .clone()
        .context("native entry omitted refresh")?;
    let key = huskarl_crypto_native::asymmetric::signer::PrivateKey::from_jwk(
        before
            .key
            .private_jwk()
            .context("missing saved private key")?
            .try_into()?,
    )?;
    let lost_grant = huskarl::grant::refresh::RefreshGrant::builder()
        .token_endpoint(format!("{origin}/oauth/token").parse()?)
        .client_id("clacks-desktop")
        .http_client(huskarl_reqwest::ReqwestClient::from(
            reqwest_oauth::Client::new(),
        ))
        .client_auth(huskarl::core::client_auth::NoAuth)
        .dpop(huskarl::core::dpop::DPoP::builder().signer(key).build())
        .build();
    use huskarl::grant::core::OAuth2ExchangeGrant;
    let lost_response = lost_grant
        .exchange(huskarl::grant::refresh::RefreshGrantParameters::refresh_token(refresh.clone()))
        .await?;
    drop(lost_response);
    drop(before);
    let expected = first.identity.clone();
    drop(first);
    let resumed = flow::resume(reference.clone()).await?;
    assert_eq!(resumed.identity, expected);
    assert_eq!(resumed.server.trim_end_matches('/'), origin);
    assert_eq!(resumed.credential_reference, reference);
    assert_eq!(credentials::load(&reference)?.refresh, Some(refresh));
    let clients: serde_json::Value = browser
        .post(format!(
            "{origin}{}",
            clacks_web::actions::clients::ListClients::PATH
        ))
        .header("origin", &origin)
        .body("")
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(
        clients.as_array().context("missing client listing")?.len(),
        1
    );
    credentials::remove(&reference)?;
    server.abort();
    Ok(())
}
