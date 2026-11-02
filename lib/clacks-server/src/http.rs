//! Axum routes and request policies for the production account application.

use crate::{auth::callbacks, runtime::ServerState};
use axum::{
    Router,
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::get,
};

/// Mount standard authentication routes using middleware-owned cookies/sessions.
pub fn router(state: ServerState, pages: Router<ServerState>) -> anyhow::Result<Router> {
    let address = state
        .config
        .public_url
        .host_str()
        .unwrap_or_default()
        .to_owned();
    let port = state
        .config
        .public_url
        .port_or_known_default()
        .map(i64::from);
    let instrumentation =
        opentelemetry_instrumentation_tower::http::server::LayerBuilder::builder()
            .with_sensitive_query_parameters(
                callbacks::Callback::SENSITIVE_QUERY_PARAMETERS
                    .iter()
                    .chain(crate::enrollment::approval::SENSITIVE_QUERY_PARAMETERS)
                    .copied(),
            )
            .with_request_extractor_fn(move |_: &Request| {
                let mut attributes = vec![opentelemetry::KeyValue::new(
                    "server.address",
                    address.clone(),
                )];
                if let Some(port) = port {
                    attributes.push(opentelemetry::KeyValue::new("server.port", port));
                }
                attributes
            })
            .build()?;
    Ok(Router::new()
        .merge(pages)
        .route("/auth/callback/{provider}", get(callbacks::callback))
        .route("/oauth/authorize", get(crate::enrollment::approval::start))
        .route_layer(middleware::from_fn_with_state(state.clone(), same_origin))
        .layer(state.sessions.clone())
        .route("/health/ready", get(ready))
        .route("/health/live", get(|| async { StatusCode::OK }))
        .merge(crate::oauth::http::router())
        .merge(crate::synchronization::router())
        .layer(middleware::from_fn(response_policy))
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                // The outer OTel layer owns context; avoid the default URI-bearing span.
                .make_span_with(tracing::Span::none())
                .on_request(())
                .on_response(
                    tower_http::trace::DefaultOnResponse::new().level(tracing::Level::INFO),
                )
                .on_failure(
                    tower_http::trace::DefaultOnFailure::new().level(tracing::Level::ERROR),
                ),
        )
        .layer(instrumentation)
        .with_state(state))
}

/// Reject cookie-authenticated browser mutations from an untrusted origin.
async fn same_origin(
    State(state): State<ServerState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        let source = if let Some(origin) = request.headers().get("origin") {
            origin
                .to_str()
                .ok()
                .and_then(|value| url::Url::parse(value).ok())
        } else {
            request
                .headers()
                .get("referer")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| url::Url::parse(value).ok())
        };
        if source.as_ref().map(url::Url::origin) != Some(state.config.public_url.origin()) {
            tracing::info!("Browser mutation origin refused");
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(request).await)
}

/// Apply non-disclosure response headers to browser and native request paths.
async fn response_policy(request: Request, next: Next) -> Response {
    let private = !request.uri().path().starts_with("/pkg/");
    let mut response = next.run(request).await;
    use axum::http::HeaderValue;
    if private {
        response
            .headers_mut()
            .insert("cache-control", HeaderValue::from_static("no-store"));
    }
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("x-frame-options", HeaderValue::from_static("DENY"));
    response
}

/// Report whether configured account storage can serve requests.
async fn ready(State(state): State<ServerState>) -> StatusCode {
    match state.store.ping().await {
        Ok(()) => StatusCode::OK,
        Err(error) => {
            tracing::error!(event="http.readiness.failed", error.type=crate::store::diagnostics::error_class(&error));
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}
