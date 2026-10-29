//! Account/code-bound secret references retained by oauth-as replay handling.

use super::keys::AccountKey;
use anyhow::Context;
use clacks_model::ids::AccountId;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Immutable binding for one consumed-code receipt.
pub(crate) struct ReceiptContext {
    /// Verified account owning the authorization code.
    pub account: AccountId,
    /// One-way lookup identity of the original code.
    pub code_digest: [u8; 32],
    /// Immutable host approval purpose associated with the code.
    pub purpose: String,
}

/// Recoverable issued references required by the library's existing replay policy.
#[derive(Serialize, Deserialize)]
pub(crate) struct IssuedReferences {
    /// Original access token, when issuance reached that point.
    pub access_token: Option<String>,
    /// Original refresh credential, when issuance produced one.
    pub refresh_token: Option<String>,
}

/// Authenticated stored receipt, without any plaintext credential fields.
pub(crate) struct EncryptedReceipt {
    /// Account-key version required to decrypt the references.
    pub key_version: i64,
    /// Fresh encryption nonce retained alongside ciphertext.
    pub nonce: Vec<u8>,
    /// Encrypted reference pair and authentication tag.
    pub ciphertext: Vec<u8>,
}

/// Protect issued references before persisting a consumed-code receipt.
pub(crate) fn seal(
    key: &AccountKey,
    context: &ReceiptContext,
    references: &IssuedReferences,
) -> anyhow::Result<EncryptedReceipt> {
    anyhow::ensure!(
        key.account == context.account,
        "receipt key belongs to another account"
    );
    let plaintext = Zeroizing::new(serde_json::to_vec(references)?);
    let associated = associated(context, key.version)?;
    let value = super::encrypt(key.bytes.as_slice(), &associated, plaintext.as_slice())?;
    Ok(EncryptedReceipt {
        key_version: key.version,
        nonce: value.nonce.to_vec(),
        ciphertext: value.bytes,
    })
}

/// Recover receipt references only within the matching account/code/purpose context.
pub(crate) fn open(
    key: &AccountKey,
    context: &ReceiptContext,
    value: &EncryptedReceipt,
) -> anyhow::Result<IssuedReferences> {
    anyhow::ensure!(
        key.account == context.account && key.version == value.key_version,
        "receipt key/context mismatch"
    );
    let nonce: [u8; 12] = value
        .nonce
        .as_slice()
        .try_into()
        .context("invalid stored receipt nonce")?;
    let associated = associated(context, key.version)?;
    let plaintext = Zeroizing::new(super::decrypt(
        key.bytes.as_slice(),
        &associated,
        nonce,
        &value.ciphertext,
    )?);
    Ok(serde_json::from_slice(plaintext.as_slice())?)
}

/// Authenticated identity of one consumed-code receipt.
#[derive(Serialize)]
struct AssociatedContext<'a> {
    /// Distinguishes this encrypted object type.
    format: &'static str,
    /// Account that approved the code.
    account: AccountId,
    /// Original one-way code identity.
    code_digest: [u8; 32],
    /// Immutable approval purpose.
    purpose: &'a str,
    /// Exact account encryption-key version.
    key_version: i64,
}

/// Encode immutable identity fields unambiguously for authenticated encryption.
fn associated(context: &ReceiptContext, version: i64) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(&AssociatedContext {
        format: "clacks.code-receipt.v1",
        account: context.account,
        code_digest: context.code_digest,
        purpose: &context.purpose,
        key_version: version,
    })?)
}
