//! One protected directory value per workspace, independent from enrollment secrets.

use anyhow::Context;
use clacks_client_store::identity::EnrollmentBinding;
use clacks_sync::protocol::directory::ProvisionedConfiguration;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Purpose-specific native namespace, independent of authorization secrets.
const SERVICE: &str = "com.w7hex.clacks-log.directory";

/// Complete protected installation, checked against the independently known owner.
#[derive(Serialize, Deserialize)]
struct Bundle {
    /// Trusted enrolled deployment origin.
    server: String,
    /// Configuration and optional login form one native value.
    installation: ProvisionedConfiguration,
}

/// Create or replace the complete installation at the workspace's fixed native entry.
///
/// This uses the backend's single-value add/update operation. There is no second
/// database commit or old native entry to clean up after credential replacement.
pub(crate) fn save(
    binding: &EnrollmentBinding,
    reference: &str,
    installation: &ProvisionedConfiguration,
) -> anyhow::Result<()> {
    installation.validate()?;
    anyhow::ensure!(
        installation.account == binding.account,
        "Directory installation belongs to another account"
    );
    let bundle = Bundle {
        server: binding.server.clone(),
        installation: installation.clone(),
    };
    let encoded = Zeroizing::new(serde_json::to_vec(&bundle)?);
    crate::credentials::entry(SERVICE, reference)?
        .set_secret(encoded.as_slice())
        .context("Could not save the protected directory configuration")
}

/// Read the whole installed value; absence is unconfigured, while access/decode failures propagate.
pub(crate) fn load(
    binding: &EnrollmentBinding,
    reference: &str,
) -> anyhow::Result<Option<ProvisionedConfiguration>> {
    let encoded = match crate::credentials::entry(SERVICE, reference)?.get_secret() {
        Ok(encoded) => Zeroizing::new(encoded),
        Err(keyring_core::Error::NoEntry) => return Ok(None),
        Err(error) => {
            return Err(error).context("Could not read protected directory configuration");
        }
    };
    let bundle: Bundle = serde_json::from_slice(&encoded)
        .context("Protected directory configuration could not be decoded")?;
    anyhow::ensure!(
        bundle.server == binding.server && bundle.installation.account == binding.account,
        "Protected directory configuration belongs to another owner"
    );
    bundle.installation.validate()?;
    Ok(Some(bundle.installation))
}

#[cfg(all(test, any(target_os = "macos", windows)))]
/// Remove only an isolated native fixture entry after its scenario.
pub(crate) fn remove(reference: &str) -> anyhow::Result<()> {
    match crate::credentials::entry(SERVICE, reference)?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(error).context("Could not remove protected directory configuration"),
    }
}
