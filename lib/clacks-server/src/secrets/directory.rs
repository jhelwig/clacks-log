//! Account/purpose/material-bound encryption of actual directory credentials.

use super::{
    envelope::{self, DirectoryEnvelope},
    keys::AccountKey,
};
use clacks_model::{
    directory::values::DirectoryProvider,
    ids::{AccountId, DirectoryMaterialId},
};
use clacks_sync::protocol::directory::DirectoryMaterial;
use serde::Serialize;
use zeroize::Zeroizing;

/// Independently expected owner and material identity, never selected by ciphertext.
#[derive(Clone, Copy, Serialize)]
pub(crate) struct DirectoryContext {
    /// Authorized account owning this credential.
    pub account: AccountId,
    /// Concrete directory purpose authenticated with the material.
    pub provider: DirectoryProvider,
    /// Stable stored credential identity.
    pub material_id: DirectoryMaterialId,
    /// Exact semantic replacement version.
    pub material_revision: i64,
}

#[cfg(test)]
mod tests;

/// Authenticate format/header along with the independently expected directory identity.
#[derive(Serialize)]
struct Associated<'a> {
    /// Distinguishes directory material from other encrypted values.
    format: &'static str,
    /// Expected storage ownership and purpose.
    context: &'a DirectoryContext,
    /// Exact cipher/key metadata.
    storage_header: String,
}

/// Encode the owning context without trusting any envelope-selected account.
fn associated(context: &DirectoryContext, version: i64) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(&Associated {
        format: "clacks.directory.v1",
        context,
        storage_header: envelope::directory_header(version),
    })?)
}

/// Encrypt actual material before the owning account transaction commits it.
pub(crate) fn seal(
    key: &AccountKey,
    context: &DirectoryContext,
    material: &DirectoryMaterial,
) -> anyhow::Result<DirectoryEnvelope> {
    anyhow::ensure!(
        key.account == context.account
            && material.id == context.material_id
            && material.revision == context.material_revision
            && material.revision > 0,
        "directory material context mismatch"
    );
    let plaintext = Zeroizing::new(serde_json::to_vec(material)?);
    let aad = associated(context, key.version())?;
    DirectoryEnvelope::new(
        key.version(),
        super::encrypt(key.bytes.as_slice(), &aad, plaintext.as_slice())?,
    )
}

/// Authenticate expected account/purpose/identity/revision before exposing login bytes.
pub(crate) fn open(
    key: &AccountKey,
    context: &DirectoryContext,
    envelope: &DirectoryEnvelope,
) -> anyhow::Result<DirectoryMaterial> {
    anyhow::ensure!(
        key.account == context.account && key.version() == envelope.key_version(),
        "directory key context mismatch"
    );
    let aad = associated(context, key.version())?;
    let plaintext = Zeroizing::new(super::decrypt(
        key.bytes.as_slice(),
        &aad,
        envelope.sealed().nonce,
        &envelope.sealed().bytes,
    )?);
    let material: DirectoryMaterial = serde_json::from_slice(plaintext.as_slice())?;
    anyhow::ensure!(
        material.id == context.material_id
            && material.revision == context.material_revision
            && material.revision > 0,
        "directory plaintext context mismatch"
    );
    Ok(material)
}
