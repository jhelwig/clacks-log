//! Real browser approval and native credentials shared by focused integration cases.

use super::provider;
use crate::enrollment::{EnrollmentRequest, EnrollmentStatus, credentials, flow};
use anyhow::Context;
use leptos::server_fn::ServerFn;
use std::sync::{Arc, atomic::AtomicU8};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Delete only the unique native entry created by this scenario, including on failure.
pub(crate) struct NativeEntry(pub(crate) String);

impl Drop for NativeEntry {
    fn drop(&mut self) {
        let _ = credentials::remove(&self.0);
    }
}

pub(crate) struct Server {
    pub(crate) origin: String,
    pub(crate) entry: NativeEntry,
    pub(crate) browser: reqwest::Client,
    pub(crate) saved_before_identity: Arc<AtomicU8>,
    provider: provider::Provider,
    subject: String,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
    _directory: tempfile::TempDir,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    pub(crate) async fn start() -> anyhow::Result<Self> {
        let provider = provider::start(None).await?;

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

        let config = serde_json::from_value(serde_json::json!({
            "public_url": origin,
            "allow_http": true,
            "database_url_file": database,
            "cookie_key_file": cookie,
            "wrapping_keys": {
                "active": "test",
                "files": {
                    "test": wrapping
                }
            },
            "providers": {
                "test": {
                    "issuer": provider.issuer,
                    "label": "Test identity provider",
                    "mode": "preconfigured",
                    "client_id": "registered-clacks"
                }
            }
        }))?;

        let state = clacks_server::runtime::ServerState::connect(config).await?;
        let options = leptos::config::LeptosOptions::builder()
            .output_name("clacks-web")
            .build();

        let reference = credentials::create(&origin)?;
        let entry = NativeEntry(reference.clone());

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
                        let saved = tokio::task::spawn_blocking(move || {
                            credentials::load(&reference)
                        })
                        .await
                        .expect("the native-entry observation task should finish")
                        .expect("the created native entry should reload at the identity request")
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

        let router =
            clacks_server::http::router(state.clone(), clacks_web::routes(state, options))?
                .layer(observer);

        let server = tokio::spawn(async move { axum::serve(listener, router).await });

        let browser = reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        Ok(Self {
            origin,
            entry,
            browser,
            saved_before_identity: native_at_identity,
            provider,
            subject: ulid::Ulid::generate().to_string(),
            task: server,
            _directory: directory,
        })
    }

    pub(crate) async fn authorize(
        &self,
        entry: &NativeEntry,
        label: &str,
    ) -> anyhow::Result<crate::enrollment::AuthorizedEnrollment> {
        let origin = self.origin.clone();
        let reference = entry.0.clone();
        let provider = &self.provider;
        let browser = &self.browser;
        let subject = self.subject.clone();

        let request = EnrollmentRequest {
            server: origin.clone(),
            label: label.into(),
            credential_reference: reference.clone(),
            expected: None,
        };

        let (status, mut progress) = watch::channel(EnrollmentStatus::default());
        let authorizing = tokio::spawn(flow::authorize(request, status, CancellationToken::new()));

        let authorization = loop {
            if progress.changed().await.is_err() {
                match authorizing.await? {
                    Err(error) => return Err(error),
                    Ok(_) => {
                        anyhow::bail!("Authorization ended before publishing its browser request")
                    }
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

        let returned = provider.authorize(&provider_request, &subject)?;
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

        Ok(first)
    }
}
