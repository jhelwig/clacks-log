//! Credential issuance and admission through the production relational issuer.

use crate::{
    auth::oidc::VerifiedIdentity,
    enrollment::{ApprovalPurpose, remember_approval, revoke_in_store},
    oauth::{config, store::OAuthStore},
    secrets::keys::WrappingKeys,
    store::AccountStore,
};
use anyhow::Context;
use http::{Method, Uri};
use huskarl_core::dpop::{AuthorizationServerDPoP, DPoP};
use huskarl_crypto_native::asymmetric::signer::{GenerateAlgorithm, PrivateKey};
use jiff::SignedDuration;
use oauth_as::{
    authorization::AuthorizationRequest,
    client::ClientId,
    server::{ClientCredential, TokenRequest, TokenRequestContext, UserApproval},
    store::Storage,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

/// Issuance details used to compare independent desktop authorizations.
pub(crate) struct IssuedClient {
    pub(crate) code: String,
    pub(crate) response: oauth_as::token::TokenResponse,
    pub(crate) actor: clacks_model::ids::ClientActorId,
}

#[tokio::test]
async fn code_exchange_activates_one_member_and_replay_revokes_only_its_grant() -> anyhow::Result<()>
{
    let _cache_guard = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .without_time()
        .with_writer(output.reopen()?)
        .finish();
    let telemetry = std::env::var_os("CLACKS_TELEMETRY_ACCEPTANCE")
        .map(|_| crate::telemetry::init(true))
        .transpose()?;
    let _guard = telemetry
        .is_none()
        .then(|| tracing::subscriber::set_default(subscriber));
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(1))
        .connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?)
        .await?;
    crate::maintenance::migrate::run(&pool).await?;
    let accounts = AccountStore::new(pool.clone());
    let login = accounts
        .provision_login(
            VerifiedIdentity {
                issuer: "https://enrollment-provider.example".into(),
                subject: ulid::Ulid::generate().to_string(),
                display_name: None,
            },
            SignedDuration::from_hours(14 * 24),
        )
        .await?;
    let principal = accounts
        .authenticate(&login, SignedDuration::from_hours(14 * 24))
        .await?
        .context("missing login")?;
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("wrapping");
    std::fs::write(&file, [31u8; 32])?;
    let keys = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "test".into(),
        files: [("test".into(), file)].into(),
    })?);
    let store = OAuthStore::new(crate::store::AccountStore::new(pool.clone()), keys);
    let issuer = config::issuer(store.clone(), &"https://clacks.example".parse()?);
    let key = PrivateKey::generate(GenerateAlgorithm::Es256, None)?;
    let thumbprint = key.as_private_jwk().thumbprint();
    let dpop = DPoP::builder().signer(key).build();
    let callback = "http://127.0.0.1:49123/oauth/callback";
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let mut issued = Vec::new();
    for label in ["First desktop", "Second desktop"] {
        issued.push(
            approve(
                &issuer,
                &store,
                &principal,
                &dpop,
                label,
                ApprovalPurpose::New,
            )
            .await?,
        );
    }

    let mut issued = issued.into_iter();
    let first = issued.next().context("missing first issuance")?;
    let second = issued.next().context("missing second issuance")?;
    assert_ne!(first.actor, second.actor);
    let recovered = approve(
        &issuer,
        &store,
        &principal,
        &dpop,
        "First desktop",
        ApprovalPurpose::RecoverActive { actor: first.actor },
    )
    .await?;
    assert_eq!(recovered.actor, first.actor);
    assert!(
        store
            .authorized(&first.response.access_token)
            .await?
            .is_none()
    );
    assert!(
        store
            .authorized(&recovered.response.access_token)
            .await?
            .is_some()
    );

    let current_token = store
        .get_token(&recovered.response.access_token)
        .await?
        .context("missing recovered access token")?;
    let mut inconsistent = current_token.as_ref().clone();
    inconsistent.grant_established_at += std::time::Duration::from_secs(1);
    assert!(store.put_token(inconsistent).await.is_err());
    assert!(
        store
            .authorized(&recovered.response.access_token)
            .await?
            .is_some()
    );

    let database_file = directory.path().join("database-url");
    let cookie_file = directory.path().join("cookies");
    std::fs::write(
        &database_file,
        std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?,
    )?;
    std::fs::write(&cookie_file, [42; 64])?;
    let server_config: crate::config::ServerConfig = serde_json::from_value(serde_json::json!({
        "public_url": "https://clacks.example", "database_url_file": database_file,
        "cookie_key_file": cookie_file, "wrapping_keys": {"active":"test", "files":{"test":directory.path().join("wrapping")}}
    }))?;
    let mut instances = Vec::new();
    let mut tasks = Vec::new();
    let mut runtime_pools = Vec::new();
    for _ in 0..2 {
        let state = crate::runtime::ServerState::connect(server_config.clone()).await?;
        runtime_pools.push(state.store.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        instances.push(format!("http://{}", listener.local_addr()?));
        let router = crate::http::router(state, axum::Router::new())?;
        tasks.push(tokio::spawn(
            async move { axum::serve(listener, router).await },
        ));
    }
    let target: Uri = "https://clacks.example/api/client".parse()?;
    let access = huskarl_core::secrets::SecretString::from(second.response.access_token.clone());
    let proof = huskarl_core::dpop::ResourceServerDPoP::proof(
        &dpop.to_resource_server_dpop(),
        &Method::GET,
        &target,
        &access,
        &thumbprint,
    )
    .await?
    .context("resource proof missing")?;
    let http = openidconnect::reqwest::Client::new();
    let instance = instances.first().context("missing application instance")?;
    let token_target: Uri = "https://clacks.example/oauth/token".parse()?;
    let refresh_proof = dpop
        .proof(&Method::POST, &token_target, Some(&thumbprint))
        .await?
        .context("refresh proof missing")?;
    let refreshed = http
        .post(format!("{instance}/oauth/token"))
        .header("dpop", refresh_proof.expose_secret())
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", "clacks-desktop"),
            (
                "refresh_token",
                second
                    .response
                    .refresh_token
                    .as_deref()
                    .context("refresh credential missing")?,
            ),
        ])
        .send()
        .await?;
    assert_eq!(refreshed.status(), http::StatusCode::OK);
    assert_eq!(
        refreshed
            .headers()
            .get("cache-control")
            .context("missing cache policy")?,
        "no-store"
    );

    let mut statuses = Vec::new();
    for instance in &instances {
        statuses.push(
            http.get(format!("{instance}/api/client"))
                .header(
                    "authorization",
                    format!("DPoP {}", second.response.access_token),
                )
                .header("dpop", proof.expose_secret())
                .send()
                .await?
                .status(),
        );
    }
    assert_eq!(
        statuses,
        [http::StatusCode::OK, http::StatusCode::UNAUTHORIZED]
    );
    let wrong_key = PrivateKey::generate(GenerateAlgorithm::Es256, None)?;
    let wrong_thumbprint = wrong_key.as_private_jwk().thumbprint();
    let wrong_dpop = DPoP::builder().signer(wrong_key).build();
    let wrong_proof = huskarl_core::dpop::ResourceServerDPoP::proof(
        &wrong_dpop.to_resource_server_dpop(),
        &Method::GET,
        &target,
        &access,
        &wrong_thumbprint,
    )
    .await?
    .context("wrong-key proof missing")?;
    assert_eq!(
        http.get(format!("{instance}/api/client"))
            .header(
                "authorization",
                format!("DPoP {}", second.response.access_token)
            )
            .header("dpop", wrong_proof.expose_secret())
            .send()
            .await?
            .status(),
        http::StatusCode::UNAUTHORIZED
    );
    let stale = store
        .get_token(&second.response.access_token)
        .await?
        .context("missing live record")?;
    assert!(
        exchange(&issuer, &dpop, &recovered.code, callback, verifier)
            .await
            .is_err()
    );
    assert!(
        store
            .authorized(&first.response.access_token)
            .await?
            .is_none()
    );
    assert!(
        store
            .authorized(&second.response.access_token)
            .await?
            .is_some()
    );
    assert!(
        crate::enrollment::client_rows(&pool, principal.account_id)
            .await?
            .iter()
            .all(|client| client.closed_at.is_none())
    );
    assert!(
        store
            .authorized(&recovered.response.access_token)
            .await?
            .is_none()
    );
    assert!(
        store
            .get_refresh_token(
                recovered
                    .response
                    .refresh_token
                    .as_deref()
                    .context("recovery refresh missing")?
            )
            .await?
            .is_none()
    );
    revoke_in_store(&pool, &principal, second.actor).await?;

    assert!(store.put_token((*stale).clone()).await?.is_refused());
    assert!(
        store
            .authorized(&second.response.access_token)
            .await?
            .is_none()
    );
    let proof = huskarl_core::dpop::ResourceServerDPoP::proof(
        &dpop.to_resource_server_dpop(),
        &Method::GET,
        &target,
        &access,
        &thumbprint,
    )
    .await?
    .context("post-revocation proof missing")?;
    assert_eq!(
        http.get(format!("{instance}/api/client"))
            .header(
                "authorization",
                format!("DPoP {}", second.response.access_token)
            )
            .header("dpop", proof.expose_secret())
            .send()
            .await?
            .status(),
        http::StatusCode::UNAUTHORIZED
    );
    // Probe semantics must not depend on browser session storage. Close only these
    // two instance pools; other tests and the main fixture pool remain available.
    for runtime_pool in runtime_pools {
        runtime_pool.close().await;
    }
    for instance in &instances {
        assert_eq!(
            http.get(format!("{instance}/health/live"))
                .send()
                .await?
                .status(),
            http::StatusCode::OK
        );
        assert_eq!(
            http.get(format!("{instance}/health/ready"))
                .send()
                .await?
                .status(),
            http::StatusCode::SERVICE_UNAVAILABLE
        );
    }
    for task in tasks {
        task.abort();
    }
    let replaced = approve(
        &issuer,
        &store,
        &principal,
        &dpop,
        "Recovered second desktop",
        ApprovalPurpose::ReplaceClosed {
            actor: second.actor,
        },
    )
    .await?;
    assert_ne!(replaced.actor, second.actor);
    let clients = crate::enrollment::client_rows(&pool, principal.account_id).await?;
    assert_eq!(clients.len(), 3);
    assert!(
        clients
            .iter()
            .any(|client| client.actor == second.actor && client.closed_at.is_some())
    );
    assert!(
        store
            .authorized(&replaced.response.access_token)
            .await?
            .is_some()
    );

    let captured = std::fs::read_to_string(output.path())?;
    for secret in [
        &first.code,
        &second.code,
        &first.response.access_token,
        &second.response.access_token,
        second
            .response
            .refresh_token
            .as_ref()
            .context("refresh missing")?,
        &thumbprint,
    ] {
        assert!(
            !captured.contains(secret),
            "credential or proof material was logged"
        );
    }
    pool.close().await;
    if let Some(telemetry) = telemetry {
        tokio::task::spawn_blocking(move || telemetry.shutdown()).await??;
    }

    Ok(())
}

pub(super) async fn exchange(
    issuer: &oauth_as::server::AuthorizationServer<OAuthStore>,
    dpop: &DPoP,
    code: &str,
    callback: &str,
    verifier: &str,
) -> anyhow::Result<oauth_as::token::TokenResponse> {
    let uri: Uri = "https://clacks.example/oauth/token".parse()?;
    let proof = dpop
        .proof(
            &Method::POST,
            &uri,
            dpop.get_current_thumbprint().await.as_deref(),
        )
        .await?
        .context("missing proof")?;
    let mut context = TokenRequestContext::new(ClientCredential::default());
    context.dpop_proof = Some(proof.expose_secret());
    Ok(issuer
        .token_with_context(
            TokenRequest::AuthorizationCode {
                client_id: ClientId::new("clacks-desktop"),
                client_secret: None,
                code: code.into(),
                redirect_uri: Some(callback.into()),
                code_verifier: Some(verifier.into()),
            },
            context,
        )
        .await?)
}

/// Approve through the actual account-bound issuer and read its committed writer.
pub(crate) async fn approve(
    issuer: &oauth_as::server::AuthorizationServer<OAuthStore>,
    store: &OAuthStore,
    principal: &crate::auth::principal::Principal,
    dpop: &DPoP,
    label: &str,
    purpose: ApprovalPurpose,
) -> anyhow::Result<IssuedClient> {
    let callback = "http://127.0.0.1:49123/oauth/callback";
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let thumbprint = dpop
        .get_current_thumbprint()
        .await
        .context("proof key missing")?;
    let request = AuthorizationRequest::from_pairs([
        ("response_type", "code".to_owned()),
        ("client_id", "clacks-desktop".to_owned()),
        ("redirect_uri", callback.to_owned()),
        ("scope", "sync".to_owned()),
        ("code_challenge_method", "S256".to_owned()),
        (
            "code_challenge",
            oauth_as::pkce::code_challenge_s256(verifier),
        ),
        ("dpop_jkt", thumbprint),
    ]);
    let validated = issuer.validate_authorization_request(&request).await?;
    let code = issuer
        .issue_authorization_code(UserApproval::granted(
            &validated,
            principal.account_id.to_string(),
        ))
        .await?;
    remember_approval(store, principal, &code.code, label, purpose).await?;
    let response = exchange(issuer, dpop, &code.code, callback, verifier).await?;
    let authorized = store
        .authorized(&response.access_token)
        .await?
        .context("issued token not activated")?;
    assert_eq!(authorized.account, principal.account_id);
    Ok(IssuedClient {
        code: code.code,
        response,
        actor: authorized.actor,
    })
}
