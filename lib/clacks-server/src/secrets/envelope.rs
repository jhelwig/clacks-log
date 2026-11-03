//! Versioned compact storage for authenticated encrypted values.

#[cfg(test)]
mod tests;

use super::Ciphertext;
use anyhow::Context;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

/// Complete protected consumed-code receipt.
pub(crate) struct ReceiptEnvelope {
    /// Exact account key referenced by the receipt.
    key_version: i64,
    /// Nonce and combined ciphertext/authentication tag.
    sealed: Ciphertext,
}

/// Complete directory credential envelope, separate from enrollment receipts.
pub(crate) struct DirectoryEnvelope {
    /// Account encryption key protecting these bytes.
    key_version: i64,
    /// Fresh nonce and authenticated ciphertext.
    sealed: Ciphertext,
}

/// Canonical directory header authenticated with the independently expected owner.
pub(crate) fn directory_header(version: i64) -> String {
    format!("$clacks$1$directory${version}")
}

impl DirectoryEnvelope {
    /// Package encrypted material under a positive account-key version.
    pub(crate) fn new(version: i64, sealed: Ciphertext) -> anyhow::Result<Self> {
        anyhow::ensure!(
            version > 0 && sealed.bytes.len() >= 16,
            "invalid encrypted directory envelope"
        );
        Ok(Self {
            key_version: version,
            sealed,
        })
    }
    /// Decode exact directory framing using the existing canonical component checks.
    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        let parts: Vec<_> = value.split('$').collect();
        let ["", "clacks", "1", "directory", version, nonce, bytes] = parts.as_slice() else {
            anyhow::bail!("unsupported or malformed directory envelope");
        };
        Self::new(key_version(version)?, ciphertext(nonce, bytes)?)
    }
    /// Encode one complete atomic storage value.
    pub(crate) fn encode(&self) -> String {
        encode(&directory_header(self.key_version), &self.sealed)
    }
    /// Return the exact account key referenced by this envelope.
    pub(crate) fn key_version(&self) -> i64 {
        self.key_version
    }
    /// Borrow encrypted bytes without exposing any plaintext.
    pub(crate) fn sealed(&self) -> &Ciphertext {
        &self.sealed
    }
}

/// Complete wrapped account key with its deployment-key reference.
pub(crate) struct WrappedKeyEnvelope {
    /// Exact deployment key ID, without delimiter interpretation.
    wrapping_key: String,
    /// Nonce and combined ciphertext/authentication tag.
    sealed: Ciphertext,
}

/// Canonical receipt metadata bound to the independent expected code context.
pub(crate) fn receipt_header(version: i64) -> String {
    format!("$clacks$1$receipt${version}")
}

/// Canonical wrapping metadata bound to the independent expected account context.
pub(crate) fn account_key_header(wrapping_key: &str) -> String {
    format!("$clacks$1$account-key${}", wrapping_reference(wrapping_key))
}

/// Encode a deployment key ID for exact generated-reference lookup.
pub(crate) fn wrapping_reference(id: &str) -> String {
    URL_SAFE_NO_PAD.encode(id.as_bytes())
}

/// Restore a deployment key ID from its generated database reference.
pub(crate) fn wrapping_id(reference: &str) -> anyhow::Result<String> {
    let id = String::from_utf8(decode(reference)?).context("wrapping key ID is not UTF-8")?;
    anyhow::ensure!(!id.is_empty(), "wrapping key ID must not be empty");
    Ok(id)
}

/// Append the nonce and authenticated ciphertext to their canonical metadata.
fn encode(header: &str, sealed: &Ciphertext) -> String {
    format!(
        "{header}${}${}",
        URL_SAFE_NO_PAD.encode(sealed.nonce),
        URL_SAFE_NO_PAD.encode(&sealed.bytes)
    )
}

/// Decode a canonical unpadded base64url component without exposing its contents.
fn decode(value: &str) -> anyhow::Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(value)
        .context("invalid encrypted-value encoding")
}

/// Decode the fixed nonce and combined ciphertext/tag framing.
fn ciphertext(nonce: &str, bytes: &str) -> anyhow::Result<Ciphertext> {
    let nonce = decode(nonce)?
        .as_slice()
        .try_into()
        .context("invalid encrypted-value nonce length")?;
    let bytes = decode(bytes)?;
    anyhow::ensure!(
        bytes.len() >= 16,
        "encrypted value lacks an authentication tag"
    );
    Ok(Ciphertext { nonce, bytes })
}

/// Decode a canonical positive account-key version.
fn key_version(value: &str) -> anyhow::Result<i64> {
    let version: i64 = value.parse().context("invalid account-key version")?;
    anyhow::ensure!(
        version > 0 && version.to_string() == value,
        "noncanonical account-key version"
    );
    Ok(version)
}

impl ReceiptEnvelope {
    /// Construct a complete receipt after authenticated encryption.
    pub(crate) fn new(key_version: i64, sealed: Ciphertext) -> anyhow::Result<Self> {
        anyhow::ensure!(key_version > 0, "receipt key version must be positive");
        anyhow::ensure!(
            sealed.bytes.len() >= 16,
            "encrypted value lacks an authentication tag"
        );
        Ok(Self {
            key_version,
            sealed,
        })
    }

    /// Restore a receipt, rejecting unsupported or ambiguous framing.
    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        let parts: Vec<_> = value.split('$').collect();
        let ["", "clacks", "1", "receipt", version, nonce, bytes] = parts.as_slice() else {
            anyhow::bail!("unsupported or malformed encrypted receipt")
        };
        Self::new(key_version(version)?, ciphertext(nonce, bytes)?)
    }

    /// Encode the entire receipt for one atomic database replacement.
    pub(crate) fn encode(&self) -> String {
        encode(&receipt_header(self.key_version), &self.sealed)
    }

    /// Return the exact account key needed for decryption.
    pub(crate) fn key_version(&self) -> i64 {
        self.key_version
    }

    /// Borrow the nonce and authenticated ciphertext for decryption.
    pub(crate) fn sealed(&self) -> &Ciphertext {
        &self.sealed
    }
}

impl WrappedKeyEnvelope {
    /// Construct a complete wrapped key with its exact deployment-key identity.
    pub(crate) fn new(wrapping_key: String, sealed: Ciphertext) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !wrapping_key.is_empty(),
            "wrapping key ID must not be empty"
        );
        anyhow::ensure!(
            sealed.bytes.len() >= 16,
            "encrypted value lacks an authentication tag"
        );
        Ok(Self {
            wrapping_key,
            sealed,
        })
    }

    /// Restore a wrapped key while preserving the exact UTF-8 deployment-key ID.
    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        let parts: Vec<_> = value.split('$').collect();
        let ["", "clacks", "1", "account-key", reference, nonce, bytes] = parts.as_slice() else {
            anyhow::bail!("unsupported or malformed wrapped account key")
        };
        let wrapping_key = wrapping_id(reference)?;
        Self::new(wrapping_key, ciphertext(nonce, bytes)?)
    }

    /// Encode the complete wrapped key for one atomic database replacement.
    pub(crate) fn encode(&self) -> String {
        encode(&account_key_header(&self.wrapping_key), &self.sealed)
    }

    /// Borrow the exact deployment-key ID required for unwrapping.
    pub(crate) fn wrapping_key(&self) -> &str {
        &self.wrapping_key
    }

    /// Borrow the nonce and authenticated ciphertext for unwrapping.
    pub(crate) fn sealed(&self) -> &Ciphertext {
        &self.sealed
    }
}
