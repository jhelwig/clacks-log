//! Account-scoped credential material with optimistic revision checks.

use super::{
    keys::{self, WrappingKeys},
    rows::CredentialRow,
};
use anyhow::Context;
use clacks_model::ids::{AccountId, CredentialId};
use serde::Serialize;
use sqlx::PgConnection;
use time::OffsetDateTime;
use zeroize::Zeroizing;

/// Immutable ownership and intended consumer of protected material.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CredentialIdentity {
    /// Authorized owning account.
    pub account: AccountId,
    /// Logical credential within the account.
    pub credential: CredentialId,
    /// Only this consumer may interpret the protected bytes.
    pub purpose: CredentialPurpose,
}

/// Supported consumers; client provisioning must use directory purposes only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum CredentialPurpose {
    /// QRZ callsign directory.
    #[serde(rename = "qrz.directory")]
    QrzDirectory,
    /// HamQTH callsign directory.
    #[serde(rename = "hamqth.directory")]
    HamqthDirectory,
    /// QRZ logbook API.
    #[serde(rename = "qrz.logbook")]
    QrzLogbook,
    /// LoTW download account.
    #[serde(rename = "lotw.account")]
    LotwAccount,
    /// LoTW signing material.
    #[serde(rename = "lotw.signing")]
    LotwSigning,
}

impl CredentialPurpose {
    /// Stable persisted purpose name, independent of diagnostic formatting.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::QrzDirectory => "qrz.directory",
            Self::HamqthDirectory => "hamqth.directory",
            Self::QrzLogbook => "qrz.logbook",
            Self::LotwAccount => "lotw.account",
            Self::LotwSigning => "lotw.signing",
        }
    }
}

/// Protected provider bytes and their owning codec version; never log this value.
pub(crate) struct SecretMaterial {
    /// Positive owner-defined codec version.
    pub payload_format: i32,
    /// Plaintext erased on release.
    pub bytes: Zeroizing<Vec<u8>>,
}

/// Public meaning of a credential independent of its encryption envelope.
#[derive(Debug)]
pub(crate) struct CredentialMetadata {
    /// Authorized identity and purpose.
    pub identity: CredentialIdentity,
    /// Material revision used to reject stale replacement/removal.
    pub material_revision: i64,
    /// Whether protected material currently exists.
    pub present: bool,
    /// Codec of present material; absent after removal.
    pub payload_format: Option<i32>,
    /// Original creation instant.
    pub created_at: OffsetDateTime,
    /// Last material replacement or removal.
    pub changed_at: OffsetDateTime,
}

/// A retained logical credential, with plaintext only when material is present.
pub(crate) struct StoredCredential {
    /// Public credential meaning.
    pub metadata: CredentialMetadata,
    /// Present material; removed credentials retain metadata alone.
    pub material: Option<SecretMaterial>,
}

/// Authenticated fields of a service-credential envelope.
#[derive(Serialize)]
struct CredentialContext {
    /// Distinguishes service material from other encrypted objects.
    format: &'static str,
    /// Immutable credential owner and purpose.
    identity: CredentialIdentityContext,
    /// Explicit material revision.
    material_revision: i64,
    /// Owner codec version.
    payload_format: i32,
    /// Exact encryption version.
    key_version: i64,
}

/// Serializable credential identity used in authenticated context.
#[derive(Serialize)]
struct CredentialIdentityContext {
    /// Owning account.
    account: AccountId,
    /// Logical credential.
    credential: CredentialId,
    /// Intended consumer.
    purpose: CredentialPurpose,
}

/// Encode immutable meaning with the exact encryption version.
fn associated(
    identity: CredentialIdentity,
    revision: i64,
    format: i32,
    version: i64,
) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(&CredentialContext {
        format: "clacks.service-credential.v1",
        identity: CredentialIdentityContext {
            account: identity.account,
            credential: identity.credential,
            purpose: identity.purpose,
        },
        material_revision: revision,
        payload_format: format,
        key_version: version,
    })?)
}

/// Select a row without disguising purpose mismatches as missing material.
async fn select(
    connection: &mut PgConnection,
    identity: CredentialIdentity,
) -> anyhow::Result<Option<CredentialRow>> {
    let row = sqlx::query_file_as!(
        CredentialRow,
        "sql/secrets/credentials/select.sql",
        identity.account.to_string(),
        identity.credential.to_string()
    )
    .fetch_optional(connection)
    .await?;
    if let Some(row) = &row {
        anyhow::ensure!(
            row.purpose == identity.purpose.as_str(),
            "credential purpose mismatch"
        );
    }
    Ok(row)
}

/// Construct public metadata after checking persisted identity and state.
fn metadata(
    identity: CredentialIdentity,
    row: &CredentialRow,
) -> anyhow::Result<CredentialMetadata> {
    anyhow::ensure!(
        row.account_id == identity.account.to_string()
            && row.credential_id == identity.credential.to_string(),
        "credential ownership mismatch"
    );
    anyhow::ensure!(
        matches!(row.state.as_str(), "present" | "removed"),
        "invalid credential state"
    );
    Ok(CredentialMetadata {
        identity,
        material_revision: row.material_revision,
        present: row.state == "present",
        payload_format: row.payload_format,
        created_at: row.created_at,
        changed_at: row.changed_at,
    })
}

/// Recover material within a caller-owned consistent read transaction.
///
/// Missing credentials return None; removed credentials retain metadata. Missing
/// keys, purpose mismatch and invalid envelopes are actionable errors.
#[tracing::instrument(skip_all, name = "account.credential.read", level = "debug", fields(account_id = %identity.account, credential_id = %identity.credential, purpose = ?identity.purpose))]
pub(crate) async fn read_in(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    identity: CredentialIdentity,
) -> anyhow::Result<Option<StoredCredential>> {
    let Some(row) = select(connection, identity).await? else {
        return Ok(None);
    };
    let metadata = metadata(identity, &row)?;
    let material = if metadata.present {
        let version = row.key_version.context("missing credential key version")?;
        let format = row.payload_format.context("missing credential format")?;
        let nonce = row.nonce.context("missing credential nonce")?;
        let ciphertext = row.ciphertext.context("missing credential ciphertext")?;
        let key = keys::load_version(&mut *connection, wrapping, identity.account, version).await?;
        let context = associated(identity, row.material_revision, format, version)?;
        let bytes = super::decrypt(
            &key.bytes,
            &context,
            nonce
                .as_slice()
                .try_into()
                .context("invalid credential nonce")?,
            &ciphertext,
        )?;
        Some(SecretMaterial {
            payload_format: format,
            bytes: Zeroizing::new(bytes),
        })
    } else {
        None
    };
    Ok(Some(StoredCredential { metadata, material }))
}

/// Create or explicitly replace material in an already account-locked transaction.
///
/// None requires absence; Some requires the exact current material revision.
/// The caller commits this change with its related provider configuration.
#[tracing::instrument(skip_all, name = "account.credential.write", level = "debug", fields(account_id = %identity.account, credential_id = %identity.credential, purpose = ?identity.purpose))]
pub(crate) async fn put_in(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    identity: CredentialIdentity,
    expected_revision: Option<i64>,
    material: SecretMaterial,
) -> anyhow::Result<CredentialMetadata> {
    anyhow::ensure!(
        material.payload_format > 0,
        "credential format must be positive"
    );
    let current = select(connection, identity).await?;
    anyhow::ensure!(
        current.as_ref().map(|row| row.material_revision) == expected_revision,
        "credential revision changed; reload before saving"
    );
    let revision = expected_revision
        .unwrap_or(0)
        .checked_add(1)
        .context("credential revision exhausted")?;
    let key = keys::active_in(connection, wrapping, identity.account).await?;
    let context = associated(identity, revision, material.payload_format, key.version)?;
    let encrypted = super::encrypt(&key.bytes, &context, &material.bytes)?;
    let changed = OffsetDateTime::now_utc();
    if current.is_some() {
        sqlx::query_file!(
            "sql/secrets/credentials/replace.sql",
            identity.account.to_string(),
            identity.credential.to_string(),
            revision,
            material.payload_format,
            key.version,
            encrypted.nonce.as_slice(),
            encrypted.bytes,
            changed
        )
        .execute(&mut *connection)
        .await?;
    } else {
        sqlx::query_file!(
            "sql/secrets/credentials/insert.sql",
            identity.account.to_string(),
            identity.credential.to_string(),
            identity.purpose.as_str(),
            revision,
            material.payload_format,
            key.version,
            encrypted.nonce.as_slice(),
            encrypted.bytes,
            changed
        )
        .execute(&mut *connection)
        .await?;
    }
    let row = select(connection, identity)
        .await?
        .context("credential write did not persist")?;
    metadata(identity, &row)
}

/// Remove current material under the caller's account lock and expected revision.
///
/// Retains logical identity and creation metadata. A stale caller cannot remove
/// newer material or later restore a removal using an obsolete revision.
#[tracing::instrument(skip_all, name = "account.credential.remove", level = "debug", fields(account_id = %identity.account, credential_id = %identity.credential, purpose = ?identity.purpose))]
pub(crate) async fn remove_in(
    connection: &mut PgConnection,
    identity: CredentialIdentity,
    expected_revision: i64,
) -> anyhow::Result<CredentialMetadata> {
    let row = select(connection, identity)
        .await?
        .context("credential does not exist")?;
    anyhow::ensure!(
        row.material_revision == expected_revision,
        "credential revision changed; reload before removal"
    );
    let revision = expected_revision
        .checked_add(1)
        .context("credential revision exhausted")?;
    sqlx::query_file!(
        "sql/secrets/credentials/remove.sql",
        identity.account.to_string(),
        identity.credential.to_string(),
        revision,
        OffsetDateTime::now_utc()
    )
    .execute(&mut *connection)
    .await?;
    let row = select(connection, identity)
        .await?
        .context("credential removal did not persist")?;
    metadata(identity, &row)
}
