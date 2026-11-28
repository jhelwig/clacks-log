//! Standards-based deterministic PKCS#12 fixture construction in Rust.

use super::certificates::{algorithm, oid};
use cbc::cipher::{BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use der::{
    Decode, Encode, Sequence, ValueOrd,
    asn1::{Any, BmpString, ObjectIdentifier, OctetString, SetOfVec},
};
use hmac::{KeyInit, Mac};
use pkcs12::{
    DigestInfo, MacData, Pfx,
    cms::{
        cert::x509::spki::AlgorithmIdentifierOwned,
        content_info::{CmsVersion, ContentInfo},
        encrypted_data::EncryptedData,
        enveloped_data::EncryptedContentInfo,
    },
    pbe_params::{EncryptedPrivateKeyInfo, Pkcs12PbeParams},
};

/// Reproducible public fixture password, never a real user secret.
pub(super) const PASSWORD: &str = "fixture-password";

/// Fixed public salt for test-only derivation and integrity operations.
const SALT: &[u8] = b"independent-clacks-fixture";

/// A standard PKCS#12 bag attribute, including deliberately misleading labels.
#[derive(Clone, Debug, Eq, PartialEq, Sequence, ValueOrd)]
struct Attribute {
    /// Attribute identifier.
    oid: ObjectIdentifier,
    /// Encoded attribute values.
    values: SetOfVec<Any>,
}

/// A standards-framed bag whose contents are explicitly constructed fixture data.
#[derive(Clone, Sequence)]
struct Bag {
    /// Standard bag-type identifier.
    id: ObjectIdentifier,
    /// Explicit bag value.
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT")]
    value: Any,
    /// Optional friendly-name/local-key-ID values.
    attributes: Option<SetOfVec<Attribute>>,
}

/// Certificate bytes wrapped as the standard X.509 certificate bag type.
#[derive(Sequence)]
struct CertificateBag {
    /// Standard certificate type.
    id: ObjectIdentifier,
    /// Original DER certificate bytes.
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT")]
    value: OctetString,
}

/// Attach fixed test labels that cannot substitute for actual public-key matching.
fn attributes(key: bool) -> anyhow::Result<SetOfVec<Attribute>> {
    Ok(SetOfVec::try_from(vec![
        Attribute {
            oid: oid("1.2.840.113549.1.9.20")?,
            values: SetOfVec::try_from(vec![Any::encode_from(&BmpString::from_utf8(
                "wrong-station",
            )?)?])?,
        },
        Attribute {
            oid: oid("1.2.840.113549.1.9.21")?,
            values: SetOfVec::try_from(vec![Any::encode_from(&OctetString::new(if key {
                b"collision".as_slice()
            } else {
                b"not-collision".as_slice()
            })?)?])?,
        },
    ])?)
}

/// Encode a certificate into its X.509 bag, retaining supplied bytes unchanged.
fn certificate(bytes: &[u8], labels: bool) -> anyhow::Result<Bag> {
    Ok(Bag {
        id: oid("1.2.840.113549.1.12.10.1.3")?,
        value: Any::encode_from(&CertificateBag {
            id: oid("1.2.840.113549.1.9.22.1")?,
            value: OctetString::new(bytes)?,
        })?,
        attributes: if labels {
            Some(attributes(false)?)
        } else {
            None
        },
    })
}

/// Wrap encoded data in a standard CMS data ContentInfo.
fn data(bytes: &[u8]) -> anyhow::Result<ContentInfo> {
    Ok(ContentInfo {
        content_type: oid("1.2.840.113549.1.7.1")?,
        content: Any::encode_from(&OctetString::new(bytes)?)?,
    })
}

/// Complete an authenticated safe with independently computed integrity bytes.
fn finish(
    safes: &[ContentInfo],
    password: &str,
    declared_rounds: Option<i32>,
    actual_rounds: i32,
    legacy: bool,
) -> anyhow::Result<Vec<u8>> {
    let auth = safes.to_vec().to_der()?;
    let mac_data = if let Some(iterations) = declared_rounds {
        let mac = if legacy {
            let key = pkcs12::kdf::derive_key_utf8::<sha1::Sha1>(
                password,
                SALT,
                pkcs12::kdf::Pkcs12KeyType::Mac,
                actual_rounds,
                20,
            )?;
            let mut mac = hmac::Hmac::<sha1::Sha1>::new_from_slice(&key)?;
            mac.update(&auth);
            DigestInfo {
                algorithm: algorithm("1.3.14.3.2.26")?,
                digest: OctetString::new(mac.finalize().into_bytes().to_vec())?,
            }
        } else {
            let key = pkcs12::kdf::derive_key_utf8::<sha2::Sha256>(
                password,
                SALT,
                pkcs12::kdf::Pkcs12KeyType::Mac,
                actual_rounds,
                32,
            )?;
            let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&key)?;
            mac.update(&auth);
            DigestInfo {
                algorithm: algorithm("2.16.840.1.101.3.4.2.1")?,
                digest: OctetString::new(mac.finalize().into_bytes().to_vec())?,
            }
        };
        Some(MacData {
            mac,
            mac_salt: OctetString::new(SALT)?,
            iterations,
        })
    } else {
        None
    };
    Ok(Pfx {
        version: pkcs12::pfx::Version::V3,
        auth_safe: data(&auth)?,
        mac_data,
    }
    .to_der()?)
}

/// Build plain bags for positive and deliberately malformed archive cases.
pub(super) fn plain(
    certs: &[Vec<u8>],
    key: &[u8],
    password: &str,
    kind: &str,
) -> anyhow::Result<Vec<u8>> {
    let bag = Bag {
        id: oid("1.2.840.113549.1.12.10.1.1")?,
        value: Any::from_der(key)?,
        attributes: if kind == "misleading-alias" {
            Some(attributes(true)?)
        } else {
            None
        },
    };
    let mut bags = vec![bag.clone()];
    if kind == "duplicate-bags" {
        bags.push(bag);
    }
    for cert in certs {
        bags.push(certificate(cert, kind == "misleading-alias")?);
    }
    let (declared, actual) = match kind {
        "no-mac" => (None, 2048),
        "mac-iterations-1000001" => (Some(1_000_001), 2048),
        "mac-iterations-0" => (Some(0), 2048),
        "mac-iterations--1" => (Some(-1), 2048),
        "mac-one-round-1" => (Some(1), 1),
        "mac-one-round-0" => (Some(0), 1),
        "mac-one-round--1" => (Some(-1), 1),
        _ => (Some(2048), 2048),
    };
    finish(&[data(&bags.to_der()?)?], password, declared, actual, false)
}

/// Encrypt a fixture value with explicit fixed parameters and no external tool.
fn encrypt(
    bytes: &[u8],
    password: &str,
    legacy: bool,
    certificate: bool,
    iterations: i32,
) -> anyhow::Result<(AlgorithmIdentifierOwned, Vec<u8>)> {
    if !legacy {
        let params = pkcs5::pbes2::Parameters::generate_pbkdf2_sha256_aes256cbc(
            iterations.try_into()?,
            SALT,
            [4; 16],
        )?;
        return Ok((
            AlgorithmIdentifierOwned {
                oid: oid("1.2.840.113549.1.5.13")?,
                parameters: Some(Any::encode_from(&params)?),
            },
            params.encrypt(password.as_bytes(), bytes)?,
        ));
    }
    let key_length = if certificate { 5 } else { 24 };
    let key = pkcs12::kdf::derive_key_utf8::<sha1::Sha1>(
        password,
        SALT,
        pkcs12::kdf::Pkcs12KeyType::EncryptionKey,
        iterations.max(1),
        key_length,
    )?;
    let iv = pkcs12::kdf::derive_key_utf8::<sha1::Sha1>(
        password,
        SALT,
        pkcs12::kdf::Pkcs12KeyType::Iv,
        iterations.max(1),
        8,
    )?;
    let encrypted = if certificate {
        cbc::Encryptor::<rc2::Rc2>::new_from_slices(&key, &iv)
            .map_err(|_| anyhow::anyhow!("invalid fixed RC2 fixture key/IV"))?
            .encrypt_padded_vec::<Pkcs7>(bytes)
    } else {
        cbc::Encryptor::<des::TdesEde3>::new_from_slices(&key, &iv)
            .map_err(|_| anyhow::anyhow!("invalid fixed DES fixture key/IV"))?
            .encrypt_padded_vec::<Pkcs7>(bytes)
    };
    Ok((
        AlgorithmIdentifierOwned {
            oid: oid(if certificate {
                "1.2.840.113549.1.12.1.6"
            } else {
                "1.2.840.113549.1.12.1.3"
            })?,
            parameters: Some(Any::encode_from(&Pkcs12PbeParams {
                salt: OctetString::new(SALT)?,
                iterations,
            })?),
        },
        encrypted,
    ))
}

/// Construct an encrypted certificate safe from explicit algorithm/ciphertext bytes.
fn encrypted_safe(
    algorithm: AlgorithmIdentifierOwned,
    ciphertext: Vec<u8>,
) -> anyhow::Result<ContentInfo> {
    Ok(ContentInfo {
        content_type: oid("1.2.840.113549.1.7.6")?,
        content: Any::encode_from(&EncryptedData {
            version: CmsVersion::V0,
            enc_content_info: EncryptedContentInfo {
                content_type: oid("1.2.840.113549.1.7.1")?,
                content_enc_alg: algorithm,
                encrypted_content: Some(OctetString::new(ciphertext)?),
            },
            unprotected_attrs: None,
        })?,
    })
}

/// Construct encrypted exports with explicit safe order and derivation counts.
///
/// Legacy counts at or below zero encode malformed parameters over deliberately
/// one-round ciphertext, for importer characterization. Normal callers use 2048.
pub(super) fn encrypted(
    cert: &[u8],
    key: &[u8],
    password: &str,
    legacy: bool,
    key_first: bool,
    iterations: i32,
) -> anyhow::Result<Vec<u8>> {
    let (algorithm, encrypted_key) = encrypt(key, password, legacy, false, iterations)?;
    let bag = Bag {
        id: oid("1.2.840.113549.1.12.10.1.2")?,
        value: Any::encode_from(&EncryptedPrivateKeyInfo {
            encryption_algorithm: algorithm,
            encrypted_data: OctetString::new(encrypted_key)?,
        })?,
        attributes: None,
    };
    let cert_bags = vec![certificate(cert, false)?].to_der()?;
    let (algorithm, ciphertext) = encrypt(&cert_bags, password, legacy, true, iterations)?;
    let key_safe = data(&vec![bag].to_der()?)?;
    let cert_safe = encrypted_safe(algorithm, ciphertext)?;
    let safes = if key_first {
        [key_safe, cert_safe]
    } else {
        [cert_safe, key_safe]
    };
    finish(&safes, password, Some(2048), 2048, legacy)
}

/// ASN.1 scrypt parameters for declared excessive work without performing it.
#[derive(Sequence)]
struct Scrypt {
    /// Fixed derivation salt.
    salt: OctetString,
    /// Declared CPU/memory cost.
    cost: u64,
    /// Declared block size.
    block: u16,
    /// Declared parallelism.
    parallel: u16,
}

/// Build authenticated excessive-work/unsupported-algorithm inputs cheaply.
pub(super) fn bounded(key: &[u8], kind: &str) -> anyhow::Result<Vec<u8>> {
    let algorithm = match kind {
        "legacy-kdf-excessive" => AlgorithmIdentifierOwned {
            oid: oid("1.2.840.113549.1.12.1.6")?,
            parameters: Some(Any::encode_from(&Pkcs12PbeParams {
                salt: OctetString::new(SALT)?,
                iterations: 1_000_001,
            })?),
        },
        "modern-kdf-excessive" => {
            let params = pkcs5::pbes2::Parameters::generate_pbkdf2_sha256_aes256cbc(
                1_000_001, SALT, [4; 16],
            )?;
            AlgorithmIdentifierOwned {
                oid: oid("1.2.840.113549.1.5.13")?,
                parameters: Some(Any::encode_from(&params)?),
            }
        }
        "scrypt-excessive-memory" | "scrypt-excessive-parallel" => {
            let (cost, parallel) = if kind == "scrypt-excessive-memory" {
                (1 << 21, 1)
            } else {
                (16, 17)
            };
            let params = pkcs12::pbe_params::Pbes2Params {
                kdf: AlgorithmIdentifierOwned {
                    oid: oid("1.3.6.1.4.1.11591.4.11")?,
                    parameters: Some(Any::encode_from(&Scrypt {
                        salt: OctetString::new(b"test".as_slice())?,
                        cost,
                        block: 8,
                        parallel,
                    })?),
                },
                encryption: AlgorithmIdentifierOwned {
                    oid: oid("2.16.840.1.101.3.4.1.42")?,
                    parameters: Some(Any::encode_from(&OctetString::new([4; 16])?)?),
                },
            };
            AlgorithmIdentifierOwned {
                oid: oid("1.2.840.113549.1.5.13")?,
                parameters: Some(Any::encode_from(&params)?),
            }
        }
        _ => AlgorithmIdentifierOwned {
            oid: oid("1.2.3.4.5")?,
            parameters: Some(Any::encode_from(&der::asn1::Null)?),
        },
    };
    let bag = Bag {
        id: oid("1.2.840.113549.1.12.10.1.1")?,
        value: Any::from_der(key)?,
        attributes: None,
    };
    finish(
        &[
            data(&vec![bag].to_der()?)?,
            encrypted_safe(algorithm, vec![0; 16])?,
        ],
        PASSWORD,
        Some(2048),
        2048,
        false,
    )
}
