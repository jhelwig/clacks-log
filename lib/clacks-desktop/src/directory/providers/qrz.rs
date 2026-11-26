//! QRZ XML directory authentication, session reuse and checked station interpretation.

use super::Authentication;
use super::{
    ProviderError, ProviderLookup, SafeProviderFailure, body, check_status, dispatched, failure,
    mapping::{self, StationFields},
    transport,
};
use clacks_model::directory::values::{DirectoryFacts, LookupKey};
use clacks_sync::protocol::directory::DirectoryMaterial;
use futures_util::FutureExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

/// One selected credential's transient QRZ session; never persisted or formatted.
#[derive(Clone)]
pub(crate) struct QrzSession {
    /// Pooled bounded transport.
    client: reqwest::Client,
    /// Fixed production endpoint or ordinary local fixture endpoint.
    endpoint: url::Url,
    /// Captured login version for this session owner.
    material: DirectoryMaterial,
    /// Last returned session key, replaced whenever the provider rotates it.
    authentication: Arc<Mutex<Authentication>>,
}

/// Decoded provider status, keeping session expiry distinct from login rejection.
enum Kind {
    /// Successful login/status without station data.
    Session,
    /// Complete successful station facts.
    Found(Box<DirectoryFacts>),
    /// Explicit unknown station response.
    NotFound,
    /// Current transient session expired.
    Expired,
}

/// One decoded response with its optional replacement session key.
struct Parsed {
    /// Provider-returned key independent of station field omissions.
    key: Option<Zeroizing<String>>,
    /// Semantic response category.
    kind: Kind,
}

impl QrzSession {
    /// Keep admitted requests on their captured session while explicit retry gets a fresh cache.
    pub(super) fn reset_authentication(&mut self) {
        self.authentication = Default::default();
    }
    /// Bind a concrete QRZ endpoint and captured credential version.
    pub(crate) fn new(
        client: reqwest::Client,
        endpoint: url::Url,
        material: DirectoryMaterial,
    ) -> Self {
        Self {
            client,
            endpoint,
            material,
            authentication: Default::default(),
        }
    }

    /// Send credentials only in an encoded POST body and retain the returned key.
    #[tracing::instrument(skip_all, name = "desktop.directory.qrz.login", level = "debug")]
    async fn login(
        client: reqwest::Client,
        endpoint: url::Url,
        material: DirectoryMaterial,
    ) -> Result<Zeroizing<String>, ProviderError> {
        let response = client
            .post(endpoint.clone())
            .form(&[
                ("username", material.login.username.as_str()),
                ("password", material.login.password.as_str()),
                (
                    "agent",
                    concat!("Clacks-Log/", env!("CLACKS_DESKTOP_VERSION")),
                ),
            ])
            .send()
            .await
            .map_err(transport)?;
        check_status(&response)?;
        let parsed = parse(&body(response).await?)?;
        if parsed.key.is_none() {
            return Err(failure(
                SafeProviderFailure::InvalidResponse,
                anyhow::anyhow!("QRZ login omitted session key"),
            ));
        }
        Ok(parsed.key.expect("checked session key"))
    }

    /// Reuse current authentication or await the same concrete login as other callers.
    async fn authenticated(&self) -> Result<Zeroizing<String>, ProviderError> {
        let login = {
            let mut authentication = self.authentication.lock().await;
            if let Some(token) = &authentication.token {
                return Ok(token.clone());
            }
            authentication
                .login
                .get_or_insert_with(|| {
                    Self::login(
                        self.client.clone(),
                        self.endpoint.clone(),
                        self.material.clone(),
                    )
                    .boxed()
                    .shared()
                })
                .clone()
        };
        let identity = login.clone();
        let result = login.await;
        let mut authentication = self.authentication.lock().await;
        if authentication
            .login
            .as_ref()
            .is_some_and(|current| current.ptr_eq(&identity))
        {
            authentication.login = None;
            if let Ok(token) = &result {
                authentication.token = Some(token.clone());
            }
        }
        result
    }

    /// Retain each successful returned key; expiry may clear only the key it actually used.
    async fn replace_used(&self, used: &str, replacement: Option<Zeroizing<String>>) {
        let mut authentication = self.authentication.lock().await;
        if replacement.is_some()
            || authentication.token.as_deref().map(|token| token.as_str()) == Some(used)
        {
            authentication.token = replacement;
        }
    }

    /// Reuse the latest key and renew an expired session at most once for this lookup.
    #[tracing::instrument(skip_all, name = "desktop.directory.qrz.lookup", level = "debug")]
    pub(crate) async fn lookup(&self, key: &LookupKey) -> Result<ProviderLookup, ProviderError> {
        for attempt in 0..2 {
            let session = self.authenticated().await?;
            dispatched("qrz");
            let response = self
                .client
                .get(self.endpoint.clone())
                .query(&[("s", session.as_str()), ("callsign", key.as_str())])
                .send()
                .await
                .map_err(transport)?;
            check_status(&response)?;
            let parsed = parse(&body(response).await?)?;
            if let Some(key) = parsed.key
                && !matches!(&parsed.kind, Kind::Expired)
            {
                self.replace_used(session.as_str(), Some(key)).await;
            }
            match parsed.kind {
                Kind::Found(facts) => return Ok(ProviderLookup::Found(facts)),
                Kind::NotFound => return Ok(ProviderLookup::NotFound),
                Kind::Expired if attempt == 0 => self.replace_used(session.as_str(), None).await,
                _ => {
                    return Err(failure(
                        SafeProviderFailure::InvalidResponse,
                        anyhow::anyhow!("QRZ lookup did not return a station result"),
                    ));
                }
            }
        }
        Err(failure(
            SafeProviderFailure::InvalidResponse,
            anyhow::anyhow!("QRZ session renewal exhausted"),
        ))
    }
}

/// Parse only documented children; unknown extensions do not become retained raw fields.
fn parse(bytes: &[u8]) -> Result<Parsed, ProviderError> {
    let text =
        std::str::from_utf8(bytes).map_err(|error| failure(SafeProviderFailure::Parse, error))?;
    let document = roxmltree::Document::parse(text)
        .map_err(|error| failure(SafeProviderFailure::Parse, error))?;
    let root = document.root_element();
    if root.tag_name().name() != "QRZDatabase" {
        return Err(failure(
            SafeProviderFailure::InvalidResponse,
            anyhow::anyhow!("Unexpected QRZ response root"),
        ));
    }
    let session = root
        .children()
        .find(|node| node.has_tag_name("Session"))
        .ok_or_else(|| {
            failure(
                SafeProviderFailure::InvalidResponse,
                anyhow::anyhow!("QRZ response omitted session status"),
            )
        })?;
    let get_session = |name| child(session, name);
    let key = get_session("Key").map(Zeroizing::new);
    if let Some(error) = get_session("Error") {
        let lower = error.to_ascii_lowercase();
        let kind = if lower.starts_with("not found") || lower.contains("callsign not found") {
            Kind::NotFound
        } else if lower.contains("session")
            && (lower.contains("timeout") || lower.contains("expired") || lower.contains("invalid"))
        {
            Kind::Expired
        } else if lower.contains("password") || lower.contains("username") {
            return Err(ProviderError::Authentication);
        } else if lower.contains("subscription") {
            return Err(failure(
                SafeProviderFailure::Subscription,
                anyhow::anyhow!("QRZ subscription restricted"),
            ));
        } else {
            return Err(failure(
                SafeProviderFailure::Refusal,
                anyhow::anyhow!("QRZ refused the request"),
            ));
        };
        return Ok(Parsed { key, kind });
    }
    let kind = if let Some(station) = root.children().find(|node| node.has_tag_name("Callsign")) {
        let get = |name| child(station, name);
        let callsign = get("call").ok_or_else(|| {
            failure(
                SafeProviderFailure::InvalidResponse,
                anyhow::anyhow!("QRZ station omitted identity"),
            )
        })?;
        let name = get("nickname").or_else(|| {
            let parts: Vec<_> = [get("fname"), get("name")].into_iter().flatten().collect();
            (!parts.is_empty()).then(|| parts.join(" "))
        });
        let coordinate_source = get("geoloc");
        let prefer_grid = coordinate_source
            .as_deref()
            .is_some_and(|source| source.eq_ignore_ascii_case("grid"));
        let facts = mapping::project(
            StationFields {
                name,
                callsign: Some(callsign),
                grid: get("grid"),
                latitude: get("lat"),
                longitude: get("lon"),
                coordinate_source,
                dxcc: get("dxcc"),
                state: get("state"),
                county: get("county"),
                cq: get("cqzone"),
                itu: get("ituzone"),
                iota: get("iota"),
                locality: get("addr2"),
            },
            prefer_grid,
        )
        .map_err(|error| failure(SafeProviderFailure::InvalidResponse, error))?;
        Kind::Found(Box::new(facts))
    } else {
        Kind::Session
    };
    Ok(Parsed { key, kind })
}

/// Read one direct text field while ignoring missing/blank optional values.
fn child(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    mapping::text(
        node.children()
            .find(|child| child.has_tag_name(name))
            .and_then(|child| child.text()),
    )
}
