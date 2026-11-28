//! Narrow provider responses used to exercise Clacks callback and registration wiring.

use axum::{
    Form, Json, Router,
    routing::{get, post},
};
use openidconnect::{
    Audience, EmptyAdditionalClaims, JsonWebKeyId, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    PrivateSigningKey, StandardClaims, SubjectIdentifier,
    core::{
        CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        CoreRsaPrivateSigningKey,
    },
};
use rsa::pkcs1::{EncodeRsaPrivateKey, LineEnding};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

pub struct Provider {
    pub issuer: openidconnect::IssuerUrl,
    signing: Arc<CoreRsaPrivateSigningKey>,
    grants: Arc<Mutex<BTreeMap<String, TokenGrant>>>,
    task: tokio::task::JoinHandle<()>,
}

struct TokenGrant {
    token: String,
    challenge: String,
}

pub struct IssuedCallback {
    pub code: String,
    pub state: String,
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Provider {
    pub fn authorize(&self, url: &url::Url, subject: &str) -> anyhow::Result<IssuedCallback> {
        let query: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        let nonce = query
            .get("nonce")
            .ok_or_else(|| anyhow::anyhow!("Clacks omitted nonce"))?;
        let state = query
            .get("state")
            .ok_or_else(|| anyhow::anyhow!("Clacks omitted state"))?;
        let challenge = query
            .get("code_challenge")
            .ok_or_else(|| anyhow::anyhow!("Clacks omitted PKCE challenge"))?;

        let now = jiff::Timestamp::now();
        let expires = now.checked_add(std::time::Duration::from_secs(5 * 60))?;
        let issued_at = std::time::SystemTime::try_from(now)?;
        let expires_at = std::time::SystemTime::try_from(expires)?;
        let claims = CoreIdTokenClaims::new(
            self.issuer.clone(),
            vec![Audience::new("registered-clacks".into())],
            expires_at.into(),
            issued_at.into(),
            StandardClaims::new(SubjectIdentifier::new(subject.to_owned())),
            EmptyAdditionalClaims {},
        )
        .set_nonce(Some(Nonce::new(nonce.to_owned())));
        let token = CoreIdToken::new(
            claims,
            self.signing.as_ref(),
            CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
            None,
            None,
        )?
        .to_string();

        let code = ulid::Ulid::generate().to_string();
        self.grants.lock().unwrap().insert(
            code.clone(),
            TokenGrant {
                token,
                challenge: challenge.clone(),
            },
        );

        Ok(IssuedCallback {
            code,
            state: state.clone(),
        })
    }
}

pub async fn start(registrations: Option<Arc<AtomicUsize>>) -> anyhow::Result<Provider> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let issuer = openidconnect::IssuerUrl::new(base.clone())?;
    let key = rsa::PrivateKey::new(&mut getrandom::SysRng, 2048)?;
    let pem = key.to_pkcs1_pem(LineEnding::LF)?;
    let signing = Arc::new(
        CoreRsaPrivateSigningKey::from_pem(&pem, Some(JsonWebKeyId::new("fixture-key".into())))
            .map_err(anyhow::Error::msg)?,
    );
    let keys = CoreJsonWebKeySet::new(vec![signing.as_verification_key()]);
    let registrations = registrations.unwrap_or_default();
    let count = registrations.clone();
    let token_registrations = registrations.clone();
    let grants: Arc<Mutex<BTreeMap<String, TokenGrant>>> = Default::default();
    let token_grants = grants.clone();
    let router = Router::new()
        .route("/.well-known/openid-configuration", get(move || {
            let base = base.clone();
            async move { Json(json!({
                "issuer": base, "authorization_endpoint": format!("{base}/authorize"),
                "token_endpoint": format!("{base}/token"), "jwks_uri": format!("{base}/keys"),
                "registration_endpoint": format!("{base}/register"),
                "response_types_supported": ["code"], "subject_types_supported": ["public"],
                "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
                "id_token_signing_alg_values_supported": ["RS256"]
            })) }
        }))
        .route("/keys", get(move || { let keys = keys.clone(); async move { Json(keys) } }))
        .route("/token", post(move |Form(form): Form<BTreeMap<String, String>>| {
            let requires_post_secret = token_registrations.load(Ordering::SeqCst) > 0;
            let grant = form.get("code").and_then(|code| token_grants.lock().unwrap().remove(code));
            async move {
                if requires_post_secret && form.get("client_secret").map(String::as_str) != Some("fixture-secret") {
                    return (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_client"})));
                }
                if let Some(grant) = grant {
                    let verifier = form.get("code_verifier").map(|value| PkceCodeVerifier::new(value.clone()));
                    if verifier.as_ref().is_some_and(|value| PkceCodeChallenge::from_code_verifier_sha256(value).as_str() == grant.challenge) {
                        return (axum::http::StatusCode::OK, Json(json!({"access_token":"fixture-access", "token_type":"Bearer", "id_token":grant.token})));
                    }
                }
                (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_grant"})))
            }
        }))
        .route("/register", post(move |Json(mut metadata): Json<Value>| {
            count.fetch_add(1, Ordering::SeqCst);
            async move {
                metadata.as_object_mut().unwrap().insert("client_id".into(), json!("registered-clacks"));
                metadata.as_object_mut().unwrap().insert("client_secret".into(), json!("fixture-secret"));
                metadata.as_object_mut().unwrap().insert("client_secret_expires_at".into(), json!(0));
                metadata.as_object_mut().unwrap().insert("token_endpoint_auth_method".into(), json!("client_secret_post"));
                (axum::http::StatusCode::CREATED, Json(metadata))
            }
        }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Ok(Provider {
        issuer,
        signing,
        grants,
        task,
    })
}
