//! Axum routes and request policies for the production account application.

use crate::{auth::callbacks, runtime::ServerState};
use axum::{
    Router,
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};

/// Mount standard authentication routes using middleware-owned cookies/sessions.
pub fn router(state: ServerState, pages: Router<ServerState>) -> Router {
    let make_span = |request: &Request| {
        let route = request
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .map(axum::extract::MatchedPath::as_str)
            .unwrap_or("unmatched");
        tracing::info_span!("http.request", method = %request.method(), route)
    };
    let on_response = |response: &Response, latency: std::time::Duration, _: &tracing::Span| {
        let status_class = match response.status().as_u16() / 100 {
            2 => "2xx",
            3 => "3xx",
            4 => "4xx",
            5 => "5xx",
            _ => "other",
        };
        metrics::counter!("clacks_http_requests_total", "status_class" => status_class)
            .increment(1);
        metrics::histogram!("clacks_http_request_duration_seconds", "status_class" => status_class)
            .record(latency.as_secs_f64());
    };
    let request_trace = tower_http::trace::TraceLayer::new_for_http()
        .make_span_with(make_span)
        .on_response(on_response);
    Router::new()
        .merge(pages)
        .route("/auth/start/{provider}", post(callbacks::start))
        .route("/auth/callback/{provider}", get(callbacks::callback))
        .route("/auth/logout", post(callbacks::logout))
        .route("/health/ready", get(ready))
        .route("/health/live", get(|| async { StatusCode::OK }))
        .route_layer(middleware::from_fn_with_state(state.clone(), same_origin))
        .layer(state.sessions.clone())
        .layer(request_trace)
        .with_state(state)
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
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(request).await)
}

/// Report whether configured account storage can serve requests.
async fn ready(State(state): State<ServerState>) -> StatusCode {
    match state.store.ping().await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
