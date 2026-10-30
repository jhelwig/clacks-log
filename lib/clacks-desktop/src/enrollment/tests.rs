//! Real native persistence and issuer interoperability; no synthetic authorization service.

#[path = "../../../clacks-web/tests/support/provider.rs"]
#[allow(dead_code)] // Shared provider support also exposes deployment-registration probes.
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
    std::fs::write(
        &database,
        std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?,
    )?;
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
    let reference = credentials::create(&origin)?;
    let _entry = NativeEntry(reference.clone());
    let native_at_identity = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
    let observed = native_at_identity.clone();
    let observed_reference = reference.clone();
    // Observe the real request boundary without changing any server response.
    let observer = axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let observed = observed.clone();
            let reference = observed_reference.clone();
            async move {
                if request.uri().path() == "/api/client" {
                    let saved = tokio::task::spawn_blocking(move || credentials::load(&reference))
                        .await
                        .unwrap()
                        .unwrap()
                        .refresh
                        .is_some();
                    let _ = observed.compare_exchange(
                        0,
                        if saved { 2 } else { 1 },
                        std::sync::atomic::Ordering::SeqCst,
                        std::sync::atomic::Ordering::SeqCst,
                    );
                }
                next.run(request).await
            }
        },
    );
    let router = clacks_server::http::router(state.clone(), clacks_web::routes(state, options))
        .layer(observer);

    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let request = EnrollmentRequest {
        server: origin.clone(),
        label: "Native persistence desktop".into(),
        credential_reference: reference.clone(),
        expected: None,
    };
    let (status, mut progress) = watch::channel(EnrollmentStatus::default());
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
    assert_eq!(
        native_at_identity.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "usable refresh credential was not saved before requesting identity"
    );
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
    let source = directory.path().join("standalone.sqlite");
    let standalone_account = clacks_model::ids::AccountId::new();
    let initiating = clacks_client_store::store::ClientStore::create_with_definitions(
        &source,
        standalone_account,
        &clacks_model::defaults::account_definitions()?,
        Default::default(),
    )
    .await?;
    let occupied = directory.path().join("occupied-account");
    let occupied_paths = crate::workspace::WorkspacePaths::at(&occupied)?;
    let other_account = clacks_model::ids::AccountId::new();
    let other = clacks_client_store::store::ClientStore::create(
        &occupied_paths.database,
        other_account,
        Default::default(),
    )
    .await?;
    let other_binding = clacks_client_store::identity::EnrollmentBinding {
        account: other_account,
        actor: clacks_model::ids::ClientActorId::new(),
        server: "https://other.example".into(),
        credential_reference: "existing-native-entry".into(),
    };
    clacks_client_store::identity::bind(&other, None, other_binding.clone()).await?;
    other.close().await?;
    let destination = directory.path().join("enrolled-account");

    // Native credential persistence can precede the SQLite phase update.
    let attempt = clacks_client_store::enrollment::EnrollmentAttempt::new(
        reference.clone(),
        origin.clone(),
        Some(occupied.clone()),
        None,
    )?;
    clacks_client_store::enrollment::save_attempt(&initiating, &attempt).await?;
    let (updates, _) = watch::channel(EnrollmentStatus::default());
    assert!(
        matches!(
            super::worker::inspect(&initiating).await?,
            EnrollmentStatus::Idle {
                discard_available: false,
                resume_available: true,
                ..
            }
        ),
        "a saved refresh credential must not be offered for discard"
    );
    let refused = super::worker::execute(
        initiating.clone(),
        crate::workspace::WorkspacePaths::at(directory.path())?,
        super::EnrollmentCommand::Discard,
        updates,
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(
        refused,
        EnrollmentStatus::Failed {
            discard_available: false,
            ..
        }
    ));
    assert_eq!(
        credentials::load(&reference)?.refresh,
        Some(refresh.clone())
    );
    assert_eq!(
        clacks_client_store::enrollment::attempt(&initiating).await?,
        Some(attempt)
    );
    initiating.close().await?;
    drop(first);
    let resumed = flow::resume(reference.clone()).await?;
    assert_eq!(resumed.identity, expected);
    assert_eq!(resumed.server.trim_end_matches('/'), origin);
    assert_eq!(resumed.credential_reference, reference);
    assert_eq!(credentials::load(&reference)?.refresh, Some(refresh));
    let initiating = clacks_client_store::store::ClientStore::open(
        &source,
        standalone_account,
        Default::default(),
    )
    .await?;
    let crate::instance::Election::Owner(owner) = crate::instance::elect(&occupied_paths)? else {
        anyhow::bail!("expected ownership of the destination")
    };
    let (notices, mut activation) = watch::channel(());
    let listener = owner.listen(&tokio::runtime::Handle::current(), notices)?;
    let error = super::completion::finish(&initiating, &resumed, CancellationToken::new())
        .await
        .err()
        .context("an open destination must not be installed")?;
    assert!(error.to_string().contains("Another Clacks process owns"));
    tokio::time::timeout(std::time::Duration::from_secs(3), activation.changed()).await??;
    let error = super::worker::restart_attempt(
        &initiating,
        &crate::workspace::WorkspacePaths::at(directory.path())?,
    )
    .await
    .err()
    .context("an open destination must not be reauthorized here")?;
    assert!(error.to_string().contains("destination workspace is open"));
    tokio::time::timeout(std::time::Duration::from_secs(3), activation.changed()).await??;
    listener.shutdown().await?;
    drop(owner);
    assert!(
        super::completion::finish(&initiating, &resumed, CancellationToken::new())
            .await
            .is_err()
    );
    let retained = clacks_client_store::enrollment::attempt(&initiating)
        .await?
        .unwrap();
    assert_eq!(
        retained.phase(),
        clacks_client_store::enrollment::EnrollmentPhase::NeedsDestination
    );
    let preserved = clacks_client_store::store::ClientStore::open(
        &occupied_paths.database,
        other_account,
        Default::default(),
    )
    .await?;
    assert_eq!(
        clacks_client_store::identity::binding(&preserved).await?,
        Some(other_binding)
    );
    preserved.close().await?;
    super::worker::change_destination(&initiating, Some(destination.clone())).await?;
    let completed =
        super::completion::finish(&initiating, &resumed, CancellationToken::new()).await?;
    assert_eq!(completed.destination, destination.canonicalize()?);
    assert!(
        clacks_client_store::identity::binding(&initiating)
            .await?
            .is_none()
    );
    assert_eq!(initiating.account_id(), standalone_account);
    let opened = clacks_client_store::store::ClientStore::open(
        &completed.destination.join("workspace.sqlite"),
        expected.account,
        Default::default(),
    )
    .await?;
    assert_eq!(
        clacks_client_store::identity::binding(&opened).await?,
        Some(completed.binding)
    );
    let mut snapshot = clacks_client_store::read::snapshot(&opened).await?;
    assert!(snapshot.checkpoint().await?.is_none());
    assert!(snapshot.account_view().await?.is_none());
    snapshot.close().await?;
    opened.close().await?;
    assert_eq!(
        clacks_client_store::enrollment::attempt(&initiating)
            .await?
            .unwrap()
            .phase(),
        clacks_client_store::enrollment::EnrollmentPhase::DestinationBound
    );
    credentials::remove(&reference)?;
    let replacement = super::worker::restart_attempt(
        &initiating,
        &crate::workspace::WorkspacePaths::at(directory.path())?,
    )
    .await?;
    let _replacement_entry = NativeEntry(replacement.credential_reference().to_owned());
    assert_ne!(replacement.credential_reference(), reference);
    let prior = replacement
        .expected()
        .context("installed destination was not retained")?;
    assert_eq!(prior.account, expected.account);
    assert_eq!(prior.actor, expected.actor);
    assert_eq!(prior.credential_reference, reference);
    assert_eq!(
        replacement.phase(),
        clacks_client_store::enrollment::EnrollmentPhase::KeySaved
    );
    assert_eq!(
        clacks_client_store::enrollment::attempt(&initiating).await?,
        Some(replacement)
    );
    assert!(
        clacks_client_store::identity::binding(&initiating)
            .await?
            .is_none()
    );
    initiating.close().await?;
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
