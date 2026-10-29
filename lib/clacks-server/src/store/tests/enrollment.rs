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
use oauth_as::{
    authorization::AuthorizationRequest,
    client::ClientId,
    server::{ClientCredential, TokenRequest, TokenRequestContext, UserApproval},
    store::Storage,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use time::Duration;

/// Issuance details used to compare independent desktop authorizations.
struct IssuedClient {
    code: String,
    response: oauth_as::token::TokenResponse,
    actor: clacks_model::ids::ClientActorId,
}

#[tokio::test]
async fn code_exchange_activates_one_member_and_replay_revokes_only_its_grant() -> anyhow::Result<()>
{
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
            Duration::days(14),
        )
        .await?;
    let principal = accounts
        .authenticate(&login, Duration::days(14))
        .await?
        .context("missing login")?;
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("wrapping");
    std::fs::write(&file, [31u8; 32])?;
    let keys = Arc::new(WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: "test".into(),
        files: [("test".into(), file)].into(),
    })?);
    let store = OAuthStore::new(pool.clone(), keys);
    let issuer = config::issuer(store.clone(), &"https://clacks.example".parse()?);
    let key = PrivateKey::generate(GenerateAlgorithm::Es256, None)?;
    let thumbprint = key.as_private_jwk().thumbprint();
    let dpop = DPoP::builder().signer(key).build();
    let callback = "http://127.0.0.1:49123/oauth/callback";
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let mut issued = Vec::new();
    for label in ["First desktop", "Second desktop"] {
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
            ("dpop_jkt", thumbprint.clone()),
        ]);
        let validated = issuer.validate_authorization_request(&request).await?;
        let code = issuer
            .issue_authorization_code(UserApproval::granted(
                &validated,
                principal.account_id.to_string(),
            ))
            .await?;
        remember_approval(&store, &principal, &code.code, label, ApprovalPurpose::New).await?;
        let response = exchange(&issuer, &dpop, &code.code, callback, verifier).await?;
        let authorized = store
            .authorized(&response.access_token)
            .await?
            .context("issued token not activated")?;
        assert_eq!(authorized.account, principal.account_id);
        issued.push(IssuedClient {
            code: code.code,
            response,
            actor: authorized.actor,
        });
    }

    let mut issued = issued.into_iter();
    let first = issued.next().context("missing first issuance")?;
    let second = issued.next().context("missing second issuance")?;
    assert_ne!(first.actor, second.actor);

    let database_file = directory.path().join("database-url");
    let cookie_file = directory.path().join("cookies");
    std::fs::write(&database_file, std::env::var("CLACKS_TEST_DATABASE_URL")?)?;
    std::fs::write(&cookie_file, [42; 64])?;
    let server_config: crate::config::ServerConfig = serde_json::from_value(serde_json::json!({
        "public_url": "https://clacks.example", "database_url_file": database_file,
        "cookie_key_file": cookie_file, "wrapping_keys": {"active":"test", "files":{"test":directory.path().join("wrapping")}}
    }))?;
    let mut instances = Vec::new();
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let state = crate::runtime::ServerState::connect(server_config.clone()).await?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        instances.push(format!("http://{}", listener.local_addr()?));
        let router = crate::http::router(state, axum::Router::new());
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
    let stale = store
        .get_token(&second.response.access_token)
        .await?
        .context("missing live record")?;
    assert!(
        exchange(&issuer, &dpop, &first.code, callback, verifier)
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
    for task in tasks {
        task.abort();
    }
    pool.close().await;
    Ok(())
}

async fn exchange(
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
