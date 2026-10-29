//! One-way database lookup values; hashes are never alternative wire credentials.

use sha2::{Digest, Sha256};

/// Credential namespaces prevent cross-kind lookup ambiguity.
#[derive(Clone, Copy)]
pub(crate) enum CredentialKind {
    /// Short-lived authorization code.
    Code,
    /// Short-lived opaque access token.
    Access,
    /// Persistent refresh credential.
    Refresh,
}

/// Hash the exact presented credential within its fixed kind namespace.
pub(crate) fn digest(kind: CredentialKind, presented: &str) -> [u8; 32] {
    let prefix: &[u8] = match kind {
        CredentialKind::Code => b"clacks.oauth.code.v1\0",
        CredentialKind::Access => b"clacks.oauth.access.v1\0",
        CredentialKind::Refresh => b"clacks.oauth.refresh.v1\0",
    };
    let mut hash = Sha256::new();
    hash.update(prefix);
    hash.update(presented.as_bytes());
    hash.finalize().into()
}
