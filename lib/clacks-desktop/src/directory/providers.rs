//! Concrete client-direct provider sessions and bounded secret-safe transport.

#[cfg(test)]
mod tests;

pub(crate) mod hamqth;
mod mapping;
pub(crate) mod qrz;

use clacks_model::directory::values::{DirectoryFacts, DirectoryProvider, LookupKey};
use clacks_sync::protocol::directory::DirectoryMaterial;
use futures_util::future::{BoxFuture, Shared};
use std::{sync::Arc, time::Duration};
use zeroize::Zeroizing;

/// An in-flight login shared by concurrent lookups using this material.
type Login = Shared<BoxFuture<'static, Result<Zeroizing<String>, ProviderError>>>;

/// Only authentication is shared; an outstanding station response never locks another lookup.
#[derive(Default)]
struct Authentication {
    /// Latest returned transient session key or token.
    token: Option<Zeroizing<String>>,
    /// One ordinary shared future coalesces concurrent login attempts.
    login: Option<Login>,
}

/// Maximum uncompressed provider payload collected by an individual response.
const MAX_BODY: usize = 1024 * 1024;

/// Actual lookup HTTP dispatches; login requests are excluded.
pub(crate) const LOOKUP_REQUESTS_METRIC: &str = "clacks_directory_lookup_requests_total";

/// Successful provider outcome; absence does not replace a retained successful snapshot.
#[derive(Debug)]
pub(crate) enum ProviderLookup {
    /// Complete returned facts, including explicit absences.
    Found(Box<DirectoryFacts>),
    /// Provider explicitly reports an unknown callsign.
    NotFound,
}

/// Bounded diagnostic categories, excluding provider response text and secret URLs.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub(crate) enum SafeProviderFailure {
    /// Request could not complete through the selected transport.
    #[error("Directory connection failed")]
    Transport,
    /// Response cannot be decoded as the documented format.
    #[error("Directory response could not be read")]
    Parse,
    /// Subscription does not permit the requested result.
    #[error("Directory subscription does not permit this lookup")]
    Subscription,
    /// Provider refuses service for this request/account.
    #[error("Directory service refused the request")]
    Refusal,
    /// Response is oversized, incomplete or inconsistent with a successful lookup.
    #[error("Directory returned an invalid response")]
    InvalidResponse,
}

/// Safe workflow error with internal sources retained only for deliberate classification.
#[derive(Clone, thiserror::Error)]
pub(crate) enum ProviderError {
    /// Credentials were rejected; runtime associates this with the captured material.
    #[error("Directory credentials were rejected")]
    Authentication,
    /// Provider asks the caller to stop until the indicated delay.
    #[error("Directory rate limit reached")]
    RateLimited {
        /// Parsed HTTP Retry-After, when provided and valid.
        retry_after: Option<Duration>,
    },
    /// Non-authentication failure that preserves prior facts.
    #[error("{reason}")]
    Unavailable {
        /// Safe displayed category.
        reason: SafeProviderFailure,
        /// Internal error, never included in default diagnostic formatting.
        source: Arc<dyn std::error::Error + Send + Sync>,
    },
}

impl std::fmt::Debug for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

/// Construct one pooled transport with bounded requests and no redirect credential forwarding.
pub(crate) fn client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Clacks-Log/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

/// Retain a safe internal error without its potentially secret request URL.
fn transport(error: reqwest::Error) -> ProviderError {
    failure(SafeProviderFailure::Transport, error.without_url())
}

/// Construct a categorized failure without retaining arbitrary provider payloads.
fn failure(reason: SafeProviderFailure, source: impl Into<anyhow::Error>) -> ProviderError {
    ProviderError::Unavailable {
        reason,
        source: source.into().into_boxed_dyn_error().into(),
    }
}

/// Interpret ordinary HTTP failures before parsing a provider payload.
fn check_status(response: &reqwest::Response) -> Result<(), ProviderError> {
    match response.status().as_u16() {
        200..=299 => Ok(()),
        401 => Err(ProviderError::Authentication),
        429 => {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| {
                    value
                        .parse::<u64>()
                        .ok()
                        .map(Duration::from_secs)
                        .or_else(|| {
                            httpdate::parse_http_date(value).ok().map(|date| {
                                date.duration_since(std::time::SystemTime::now())
                                    .unwrap_or_default()
                            })
                        })
                });
            Err(ProviderError::RateLimited { retry_after })
        }
        _ => Err(failure(
            SafeProviderFailure::Refusal,
            anyhow::anyhow!("Provider HTTP request refused"),
        )),
    }
}

/// Collect within the response budget, including responses without Content-Length.
async fn body(mut response: reqwest::Response) -> Result<Vec<u8>, ProviderError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BODY as u64)
    {
        return Err(failure(
            SafeProviderFailure::InvalidResponse,
            anyhow::anyhow!("Provider body exceeds response limit"),
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        if chunk.len() > MAX_BODY.saturating_sub(body.len()) {
            return Err(failure(
                SafeProviderFailure::InvalidResponse,
                anyhow::anyhow!("Provider body exceeds response limit"),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Count actual lookup HTTP attempts, excluding login and independent of logical outcomes.
fn dispatched(provider: &'static str) {
    metrics::counter!(LOOKUP_REQUESTS_METRIC, "provider" => provider).increment(1);
}
/// Own exactly the selected concrete session; there is no fallback dispatch.
#[derive(Clone)]
pub(crate) enum ProviderSession {
    /// QRZ XML session.
    Qrz(qrz::QrzSession),
    /// HamQTH JSON bearer session.
    HamQth(hamqth::HamQthSession),
}

impl ProviderSession {
    /// Explicit authentication retry starts fresh transient authentication for new requests.
    pub(crate) fn reset_authentication(&mut self) {
        match self {
            Self::Qrz(session) => session.reset_authentication(),
            Self::HamQth(session) => session.reset_authentication(),
        }
    }
    /// Bind protected material to the selected fixed production endpoint.
    pub(crate) fn new(
        provider: DirectoryProvider,
        material: DirectoryMaterial,
    ) -> anyhow::Result<Self> {
        let client = client()?;
        Ok(match provider {
            DirectoryProvider::Qrz => Self::Qrz(qrz::QrzSession::new(
                client,
                "https://xmldata.qrz.com/xml/current/".parse()?,
                material,
            )),
            DirectoryProvider::HamQth => Self::HamQth(hamqth::HamQthSession::new(
                client,
                "https://www.hamqth.com/api/v2/".parse()?,
                material,
            )),
        })
    }

    /// Dispatch only to this owned session and retain its transient authentication state.
    pub(crate) async fn lookup(&self, key: &LookupKey) -> Result<ProviderLookup, ProviderError> {
        match self {
            Self::Qrz(session) => session.lookup(key).await,
            Self::HamQth(session) => session.lookup(key).await,
        }
    }
}
