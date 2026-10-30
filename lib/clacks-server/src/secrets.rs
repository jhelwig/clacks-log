//! Account-owned secret envelopes, independent of browser and provider keys.

pub(crate) mod envelope;
pub(crate) mod keys;
pub(crate) mod receipt;
pub(crate) mod rows;

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use rand::{RngCore, rngs::OsRng};

/// Authenticated ciphertext with the fresh nonce needed for decryption.
pub(crate) struct Ciphertext {
    /// Random nonce generated for this encryption only.
    pub nonce: [u8; 12],
    /// Ciphertext including the authentication tag.
    pub bytes: Vec<u8>,
}

/// Encrypt one account-bound value using the library's authenticated cipher.
fn encrypt(key: &[u8], associated: &[u8], value: &[u8]) -> anyhow::Result<Ciphertext> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| anyhow::anyhow!("invalid encryption key length"))?;
    let mut nonce = [0u8; 12];
    OsRng.try_fill_bytes(&mut nonce)?;
    let bytes = cipher
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: value,
                aad: associated,
            },
        )
        .map_err(|_| anyhow::anyhow!("secret encryption failed"))?;
    Ok(Ciphertext { nonce, bytes })
}

/// Authenticate an envelope before exposing any plaintext to its owning operation.
fn decrypt(
    key: &[u8],
    associated: &[u8],
    nonce: [u8; 12],
    bytes: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| anyhow::anyhow!("invalid encryption key length"))?;
    cipher
        .decrypt(
            &Nonce::from(nonce),
            Payload {
                msg: bytes,
                aad: associated,
            },
        )
        .map_err(|_| {
            anyhow::anyhow!(
                "secret decryption failed; verify retained wrapping keys and account context"
            )
        })
}
