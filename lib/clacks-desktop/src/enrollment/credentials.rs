//! Native credential entries containing signing material and complete refresh metadata.

use anyhow::Context;
use huskarl::{core::jwk::Jwk, token::RefreshToken};
use huskarl_crypto_native::asymmetric::signer::{GenerateAlgorithm, PrivateKey};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Native-store namespace, separate from ordinary application preferences.
const SERVICE: &str = "org.clacks-log.desktop.authorization";

/// Secret-bearing native entry; never format it for logs or UI state.
#[derive(Serialize, Deserialize)]
pub(crate) struct CredentialBundle {
    /// Selected deployment origin, not a callback-supplied endpoint.
    pub server: String,
    /// Server-owned public application registration.
    pub client_id: String,
    /// Persisted private JSON Web Key material.
    pub key: Jwk,
    /// Complete Huskarl refresh metadata, absent until browser exchange succeeds.
    pub refresh: Option<RefreshToken>,
}

/// Generate and durably save a fresh proof key before opening the browser.
pub(crate) fn create(server: &str) -> anyhow::Result<String> {
    let key = PrivateKey::generate(GenerateAlgorithm::Es256, None)?;
    let reference = ulid::Ulid::generate().to_string();
    let bundle = CredentialBundle {
        server: server.into(),
        client_id: "clacks-desktop".into(),
        key: key.as_private_jwk().into(),
        refresh: None,
    };
    let encoded = Zeroizing::new(serde_json::to_vec(&bundle)?);
    crate::credentials::entry(SERVICE, &reference)?
        .set_secret(encoded.as_slice())
        .context("Could not save the desktop signing key in the native credential store")?;
    Ok(reference)
}

/// Load one known native entry for authorization or persistent credential recovery.
pub(crate) fn load(reference: &str) -> anyhow::Result<CredentialBundle> {
    let encoded = Zeroizing::new(crate::credentials::entry(SERVICE, reference)?.get_secret().context(
        "Could not read the desktop credential; authorize this client again if it is missing",
    )?);
    serde_json::from_slice(&encoded).context("The native desktop credential could not be decoded")
}

/// Persist the complete successful refresh result without dropping library metadata.
pub(crate) fn save_refresh(reference: &str, refresh: RefreshToken) -> anyhow::Result<()> {
    let mut bundle = load(reference)?;
    bundle.refresh = Some(refresh);
    let encoded = Zeroizing::new(serde_json::to_vec(&bundle)?);
    crate::credentials::entry(SERVICE, reference)?
        .set_secret(encoded.as_slice())
        .context("Could not save the desktop refresh credential in the native credential store")?;
    Ok(())
}

/// Remove a superseded entry only after its replacement is durable, or clean an abandoned key.
pub(crate) fn remove(reference: &str) -> anyhow::Result<()> {
    match crate::credentials::entry(SERVICE, reference)?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(error).context("Could not remove the superseded native credential"),
    }
}
