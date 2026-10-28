//! Only the provider endpoints needed by Clacks account scenarios.

use axum::{
    Json, Router,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub struct Provider {
    pub issuer: openidconnect::IssuerUrl,
    pub registrations: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn start() -> anyhow::Result<Provider> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let issuer = openidconnect::IssuerUrl::new(base.clone())?;
    let registrations = Arc::new(AtomicUsize::new(0));
    let count = registrations.clone();
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
        .route("/keys", get(|| async { Json(json!({"keys": []})) }))
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
        task,
    })
}
