//! Standard Huskarl discovery, callback, refresh and proof-bound identity requests.

use super::{
    AuthorizedEnrollment, EnrollmentRequest, EnrollmentStatus,
    credentials::{self, CredentialBundle},
};
use anyhow::Context;
use http::{Method, Uri};
use huskarl::{
    authorizer::HttpAuthorizer,
    cache::{GrantTokenSource, InMemoryRefreshTokenStore, InMemoryTokenCache, NoSource},
    core::{client_auth::NoAuth, dpop::DPoP, server_metadata::AuthorizationServerMetadata},
    grant::{
        authorization_code::{AuthorizationCodeGrant, StartInput, bind_loopback},
        core::OAuth2ExchangeGrant,
        refresh::{RefreshGrant, RefreshGrantParameters},
    },
    token::AccessToken,
};
use huskarl_crypto_native::asymmetric::signer::PrivateKey;
use huskarl_reqwest::ReqwestClient;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Validate a deployment origin; plaintext is restricted to explicit loopback development.
pub(crate) fn origin(value: &str) -> anyhow::Result<url::Url> {
    let url = url::Url::parse(value).context("Enter the server's HTTPS origin")?;
    let local = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain("localhost")) => true,
        _ => false,
    };
    anyhow::ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && local),
        "The server requires HTTPS; HTTP is allowed only for loopback development"
    );
    anyhow::ensure!(
        url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none(),
        "Enter only the server origin, without credentials, a path, query or fragment"
    );
    Ok(url)
}

/// Reconstruct the selected native signing key using Huskarl's own JWK conversion.
fn signer(bundle: &CredentialBundle) -> anyhow::Result<PrivateKey> {
    PrivateKey::from_jwk(
        bundle
            .key
            .private_jwk()
            .context("The saved credential has no private signing key")?
            .try_into()?,
    )
    .map_err(Into::into)
}

/// Build bounded, redirect-disabled Rustls transport for issuer and resource calls.
fn transport() -> anyhow::Result<reqwest_oauth::Client> {
    Ok(reqwest_oauth::Client::builder()
        .redirect(reqwest_oauth::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()?)
}

/// Complete explicit browser authorization using the already durable proof key.
#[tracing::instrument(skip_all, name = "desktop.enrollment.authorize")]
pub(crate) async fn authorize(
    request: EnrollmentRequest,
    status: watch::Sender<EnrollmentStatus>,
    cancellation: CancellationToken,
) -> anyhow::Result<AuthorizedEnrollment> {
    let selected = origin(&request.server)?.to_string();
    let reference = request.credential_reference.clone();
    let bundle = tokio::task::spawn_blocking(move || credentials::load(&reference)).await??;
    anyhow::ensure!(
        origin(&bundle.server)?.as_str() == selected,
        "The saved key belongs to a different server"
    );
    let http = transport()?;
    let adapter = ReqwestClient::from(http.clone());
    let metadata = tokio::select! {
        _ = cancellation.cancelled() => anyhow::bail!("Authorization cancelled; the saved attempt is available for retry"),
        result = AuthorizationServerMetadata::fetch().http_client(&adapter).issuer(selected.trim_end_matches('/')).call() => result?,
    };
    let listener = bind_loopback(0).await?;
    let callback = format!("http://{}/oauth/callback", listener.local_addr()?);
    let key = signer(&bundle)?;
    let grant = AuthorizationCodeGrant::builder_from_metadata(&metadata)?
        .client_id(bundle.client_id.clone())
        .http_client(adapter)
        .client_auth(NoAuth)
        .redirect_uri(callback)
        .oidc(false)
        .dpop(DPoP::builder().signer(key.clone()).build())
        .build()
        .await?;
    let start = grant.start(StartInput::scope(vec!["sync".into()])).await?;
    let mut authorization = url::Url::parse(&start.authorization_url.to_string())?;
    authorization
        .query_pairs_mut()
        .append_pair("client_label", request.label.trim());
    if let Some(expected) = &request.expected {
        authorization
            .query_pairs_mut()
            .append_pair("clacks_account", &expected.account.to_string())
            .append_pair("clacks_actor", &expected.actor.to_string());
    }
    status.send_replace(EnrollmentStatus::Authorizing {
        authorization_url: authorization.to_string(),
    });
    let completed = tokio::select! {
        _ = cancellation.cancelled() => anyhow::bail!("Authorization cancelled. A completed remote approval is not undone."),
        result = grant.complete_on_loopback(&listener,&start.pending_state,None) => result?,
    };
    status.send_replace(EnrollmentStatus::Saving);
    finish(
        &request.credential_reference,
        bundle,
        key,
        http,
        completed.token_response,
        request.expected.as_ref(),
    )
    .await
}

/// Refresh from the saved origin/client/key without another browser authorization.
#[tracing::instrument(skip_all, name = "desktop.enrollment.resume")]
pub(crate) async fn resume(reference: String) -> anyhow::Result<AuthorizedEnrollment> {
    let load_reference = reference.clone();
    let bundle = tokio::task::spawn_blocking(move || credentials::load(&load_reference)).await??;
    origin(&bundle.server)?;
    let key = signer(&bundle)?;
    let refresh = bundle
        .refresh
        .clone()
        .context("Browser authorization is required to finish this saved attempt")?;
    anyhow::ensure!(
        refresh.dpop_jkt() == Some(key.as_private_jwk().thumbprint().as_str()),
        "The saved refresh credential does not match its proof key"
    );
    let http = transport()?;
    let grant = refresh_grant(&bundle, &http, key.clone())?;
    let response = grant
        .exchange(RefreshGrantParameters::refresh_token(refresh))
        .await?;
    finish(&reference, bundle, key, http, response, None).await
}

/// Compose the standard refresh grant from the saved deployment metadata.
fn refresh_grant(
    bundle: &CredentialBundle,
    http: &reqwest_oauth::Client,
    key: PrivateKey,
) -> anyhow::Result<RefreshGrant> {
    Ok(RefreshGrant::builder()
        .token_endpoint(format!("{}oauth/token", origin(&bundle.server)?).parse()?)
        .client_id(bundle.client_id.clone())
        .http_client(ReqwestClient::from(http.clone()))
        .client_auth(NoAuth)
        .dpop(DPoP::builder().signer(key).build())
        .build())
}

/// Verify proof bindings and protected identity before making successful credentials durable.
async fn finish(
    reference: &str,
    bundle: CredentialBundle,
    key: PrivateKey,
    http: reqwest_oauth::Client,
    response: huskarl::grant::core::TokenResponse,
    expected: Option<&clacks_client_store::identity::EnrollmentBinding>,
) -> anyhow::Result<AuthorizedEnrollment> {
    let thumbprint = key.as_private_jwk().thumbprint();
    let AccessToken::DPoP(access) = response.access_token() else {
        anyhow::bail!("The server returned an access token without proof binding");
    };
    anyhow::ensure!(
        access.jkt() == thumbprint,
        "The issued access token has the wrong proof binding"
    );
    let refresh = response
        .refresh_token()
        .context("The server did not issue a persistent refresh credential")?
        .clone();
    anyhow::ensure!(
        refresh.dpop_jkt() == Some(thumbprint.as_str()),
        "The issued refresh credential has the wrong proof binding"
    );
    let saved_reference = reference.to_owned();
    tokio::task::spawn_blocking(move || credentials::save_refresh(&saved_reference, refresh))
        .await??;

    let source = GrantTokenSource::builder()
        .grant(refresh_grant(&bundle, &http, key)?)
        .grant_parameters(NoSource)
        .refresh_store(InMemoryRefreshTokenStore::default())
        .build();
    source.prime(response).await?;
    let authorizer = HttpAuthorizer::builder()
        .cache(InMemoryTokenCache::builder().source(source).build())
        .build();
    let target: Uri = format!("{}api/client", origin(&bundle.server)?).parse()?;
    let headers = authorizer.get_headers(&Method::GET, &target).await?;
    let identity: clacks_sync::protocol::ClientIdentity = http
        .get(target.to_string())
        .headers(headers)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(expected) = expected {
        anyhow::ensure!(
            identity.account == expected.account,
            "The authorized server account does not match this workspace"
        );
    }
    Ok(AuthorizedEnrollment {
        identity,
        server: origin(&bundle.server)?.to_string(),
        credential_reference: reference.into(),
    })
}
