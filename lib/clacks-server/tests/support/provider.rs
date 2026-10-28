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
    pub registrations: Arc<AtomicUsize>,
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
        self.authorize_at(url, subject, chrono::Utc::now())
    }

    pub fn authorize_at(
        &self,
        url: &url::Url,
        subject: &str,
        authenticated_at: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<IssuedCallback> {
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
        let now = chrono::Utc::now();
        let claims = CoreIdTokenClaims::new(
            self.issuer.clone(),
            vec![Audience::new("registered-clacks".into())],
            now + chrono::Duration::minutes(5),
            now,
            StandardClaims::new(SubjectIdentifier::new(subject.to_owned())),
            EmptyAdditionalClaims {},
        )
        .set_nonce(Some(Nonce::new(nonce.to_owned())))
        .set_auth_time(Some(authenticated_at));
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

pub async fn start() -> anyhow::Result<Provider> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let issuer = openidconnect::IssuerUrl::new(base.clone())?;
    let key = rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)?;
    let pem = key.to_pkcs1_pem(LineEnding::LF)?;
    let signing = Arc::new(
        CoreRsaPrivateSigningKey::from_pem(&pem, Some(JsonWebKeyId::new("fixture-key".into())))
            .map_err(anyhow::Error::msg)?,
    );
    let keys = CoreJsonWebKeySet::new(vec![signing.as_verification_key()]);
    let registrations = Arc::new(AtomicUsize::new(0));
    let count = registrations.clone();
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
                "id_token_signing_alg_values_supported": ["RS256"]
            })) }
        }))
        .route("/keys", get(move || { let keys = keys.clone(); async move { Json(keys) } }))
        .route("/token", post(move |Form(form): Form<BTreeMap<String, String>>| {
            let grant = form.get("code").and_then(|code| token_grants.lock().unwrap().remove(code));
            async move {
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
                (axum::http::StatusCode::CREATED, Json(metadata))
            }
        }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Ok(Provider {
        issuer,
        registrations,
        signing,
        grants,
        task,
    })
}
