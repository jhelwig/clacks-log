//! HamQTH v2 JSON login and bearer-token callbook lookup.

use super::Authentication;
use super::{
    ProviderError, ProviderLookup, SafeProviderFailure, body, check_status, dispatched, failure,
    mapping::{self, StationFields},
    transport,
};
use clacks_model::directory::values::LookupKey;
use clacks_sync::protocol::directory::DirectoryMaterial;
use futures_util::FutureExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

/// One selected credential's transient HamQTH authentication state.
#[derive(Clone)]
pub(crate) struct HamQthSession {
    /// Pooled bounded transport.
    client: reqwest::Client,
    /// Fixed API base or ordinary fixture base, always ending with a slash.
    endpoint: url::Url,
    /// Captured protected login version.
    material: DirectoryMaterial,
    /// Current bearer token, never persisted or formatted.
    authentication: Arc<Mutex<Authentication>>,
}

impl HamQthSession {
    /// Keep admitted requests on their captured session while explicit retry gets a fresh cache.
    pub(super) fn reset_authentication(&mut self) {
        self.authentication = Default::default();
    }
    /// Bind the concrete API base and credential version to one session owner.
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

    /// Send login credentials only as a POST body and retain the token for reuse.
    #[tracing::instrument(skip_all, name = "desktop.directory.hamqth.login", level = "debug")]
    async fn login(
        client: reqwest::Client,
        endpoint: url::Url,
        material: DirectoryMaterial,
    ) -> Result<Zeroizing<String>, ProviderError> {
        let url = endpoint
            .join("login")
            .map_err(|error| failure(SafeProviderFailure::InvalidResponse, error))?;
        let response = client.post(url).json(&serde_json::json!({"callsign":material.login.username,"password":material.login.password})).send().await.map_err(transport)?;
        check_status(&response)?;
        let value: serde_json::Value = serde_json::from_slice(&body(response).await?)
            .map_err(|error| failure(SafeProviderFailure::Parse, error))?;
        let token = value
            .get("token")
            .and_then(serde_json::Value::as_str)
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| {
                failure(
                    SafeProviderFailure::InvalidResponse,
                    anyhow::anyhow!("HamQTH login omitted token"),
                )
            })?;
        Ok(Zeroizing::new(token.to_owned()))
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

    /// A response may replace only the transient authentication it actually used.
    async fn replace_used(&self, used: &str, replacement: Option<Zeroizing<String>>) {
        let mut authentication = self.authentication.lock().await;
        if authentication.token.as_deref().map(|token| token.as_str()) == Some(used) {
            authentication.token = replacement;
        }
    }

    /// Reuse bearer authentication, with a single renewal/repeat after lookup expiry.
    #[tracing::instrument(skip_all, name = "desktop.directory.hamqth.lookup", level = "debug")]
    pub(crate) async fn lookup(&self, key: &LookupKey) -> Result<ProviderLookup, ProviderError> {
        let mut url = self
            .endpoint
            .join("callbook/")
            .map_err(|error| failure(SafeProviderFailure::InvalidResponse, error))?;
        url.path_segments_mut()
            .map_err(|_| {
                failure(
                    SafeProviderFailure::InvalidResponse,
                    anyhow::anyhow!("HamQTH endpoint cannot hold a callsign path"),
                )
            })?
            .pop_if_empty()
            .push(key.as_str());
        url.query_pairs_mut()
            .append_pair("app", concat!("Clacks-Log/", env!("CARGO_PKG_VERSION")));
        for attempt in 0..2 {
            let token = self.authenticated().await?;
            dispatched("hamqth");
            let response = self
                .client
                .get(url.clone())
                .bearer_auth(token.as_str())
                .send()
                .await
                .map_err(transport)?;
            match response.status().as_u16() {
                401 if attempt == 0 => {
                    self.replace_used(token.as_str(), None).await;
                    continue;
                }
                401 => {
                    self.replace_used(token.as_str(), None).await;
                    return Err(failure(
                        SafeProviderFailure::InvalidResponse,
                        anyhow::anyhow!("HamQTH rejected the renewed session"),
                    ));
                }
                404 => return Ok(ProviderLookup::NotFound),
                _ => check_status(&response)?,
            }
            let value: serde_json::Value = serde_json::from_slice(&body(response).await?)
                .map_err(|error| failure(SafeProviderFailure::Parse, error))?;
            let get = |name| field(&value, name);
            let callsign = get("callsign").ok_or_else(|| {
                failure(
                    SafeProviderFailure::InvalidResponse,
                    anyhow::anyhow!("HamQTH station omitted identity"),
                )
            })?;
            let location = value.get("location");
            let region = value.get("region");
            let facts = mapping::project(
                StationFields {
                    name: get("nick").or_else(|| get("name")),
                    callsign: Some(callsign),
                    grid: location.and_then(|value| field(value, "grid")),
                    latitude: location.and_then(|value| field(value, "lat")),
                    longitude: location.and_then(|value| field(value, "lon")),
                    dxcc: get("dxcc"),
                    state: region.and_then(|value| field(value, "usState")),
                    county: region.and_then(|value| field(value, "usCounty")),
                    cq: get("cq"),
                    itu: get("itu"),
                    iota: get("iota"),
                    locality: get("qth"),
                    coordinate_source: None,
                },
                true,
            )
            .map_err(|error| failure(SafeProviderFailure::InvalidResponse, error))?;
            return Ok(ProviderLookup::Found(Box::new(facts)));
        }
        Err(failure(
            SafeProviderFailure::InvalidResponse,
            anyhow::anyhow!("HamQTH session renewal exhausted"),
        ))
    }
}

/// Read documented scalar fields without retaining unsupported JSON or privacy omissions.
fn field(value: &serde_json::Value, name: &str) -> Option<String> {
    match value.get(name)? {
        serde_json::Value::String(value) => mapping::text(Some(value)),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}
