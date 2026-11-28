//! Literal X.509 structures for reproducible synthetic certificate cases.

use der::{
    Encode, Sequence, ValueOrd,
    asn1::{
        Any, BitString, ContextSpecific, GeneralizedTime, ObjectIdentifier, OctetString, SetOfVec,
        Uint, UtcTime, Utf8StringRef,
    },
};
use pkcs12::cms::cert::x509::spki::AlgorithmIdentifierOwned;
use rand_chacha::ChaCha20Rng;
use rsa::rand_core::SeedableRng;
use rsa::{
    PrivateKey,
    pkcs8::EncodePublicKey,
    signature::{RandomizedSigner, SignatureEncoding},
};

/// One X.509 distinguished-name attribute, permitting intentional malformed names.
#[derive(Clone, Debug, Eq, PartialEq, Sequence, ValueOrd)]
struct Attribute {
    /// Attribute object identifier.
    oid: ObjectIdentifier,
    /// Typed directory-string value.
    value: Any,
}

/// A distinguished name in its literal relative-name order.
type Name = Vec<SetOfVec<Attribute>>;

/// Certificate validity independent of wall-clock time.
#[derive(Clone, Sequence)]
struct Validity {
    /// Fixed beginning, 2020-01-01 UTC.
    not_before: UtcTime,
    /// UTC or generalized expiry encoding selected by the certificate recipe.
    not_after: Any,
}

/// A certificate extension, including deliberately duplicated protocol fields.
#[derive(Clone, Sequence)]
struct Extension {
    /// Extension identifier.
    oid: ObjectIdentifier,
    /// Whether unknown readers must reject the extension.
    #[asn1(default = "Default::default")]
    critical: bool,
    /// Complete raw extension contents.
    value: OctetString,
}

/// Standards-encoded certificate body with explicit fixture-controlled values.
#[derive(Sequence)]
struct Body {
    /// Explicit version 3 declaration.
    version: ContextSpecific<u8>,
    /// Full fixed serial bytes.
    serial: Uint,
    /// Signature algorithm, SHA-256 with RSA.
    signature: AlgorithmIdentifierOwned,
    /// Synthetic root name.
    issuer: Name,
    /// Fixed validity period.
    validity: Validity,
    /// Synthetic station subject.
    subject: Name,
    /// Complete encoded public-key information.
    public_key: Any,
    /// Explicit extension list, including intentionally invalid cases.
    extensions: ContextSpecific<Vec<Extension>>,
}

/// Full certificate with independently signed body bytes.
#[derive(Sequence)]
struct Certificate {
    /// Signed certificate body.
    body: Body,
    /// Body signature algorithm.
    algorithm: AlgorithmIdentifierOwned,
    /// Actual RSA signature bits.
    signature: BitString,
}

/// Construct a versioned explicit context wrapper for an ASN.1 fixture field.
fn explicit<T>(number: u32, value: T) -> ContextSpecific<T> {
    ContextSpecific {
        tag_number: der::TagNumber(number),
        tag_mode: der::TagMode::Explicit,
        value,
    }
}

/// Parse a fixed protocol object identifier, propagating encoding errors.
pub(super) fn oid(value: &str) -> anyhow::Result<ObjectIdentifier> {
    Ok(value.parse()?)
}

/// Build a standard algorithm identifier with explicit NULL parameters.
pub(super) fn algorithm(value: &str) -> anyhow::Result<AlgorithmIdentifierOwned> {
    Ok(AlgorithmIdentifierOwned {
        oid: oid(value)?,
        parameters: Some(Any::encode_from(&der::asn1::Null)?),
    })
}

/// Assemble directory attributes without native callsign normalization.
fn name(values: &[(&str, &str)]) -> anyhow::Result<Name> {
    values
        .iter()
        .map(|(id, value)| {
            Ok(SetOfVec::try_from(vec![Attribute {
                oid: oid(id)?,
                value: if *id == "1.2.840.113549.1.9.1" {
                    Any::encode_from(&der::asn1::Ia5StringRef::new(value)?)?
                } else {
                    Any::encode_from(&Utf8StringRef::new(value)?)?
                },
            }])?)
        })
        .collect()
}

/// Encode one raw LoTW extension value, without an extra nested string encoding.
fn extension(id: &str, value: &[u8]) -> anyhow::Result<Extension> {
    Ok(Extension {
        oid: oid(id)?,
        critical: false,
        value: OctetString::new(value)?,
    })
}

/// Build a synthetic station or named malformed variant using only Rust values.
pub(super) fn station(
    key: &PrivateKey,
    issuer: &PrivateKey,
    variant: &str,
) -> anyhow::Result<Vec<u8>> {
    let call = match variant {
        "unicode-expands" => "N0CAß",
        "unicode-remains" => "N0CAØ",
        "control-call" => "N0CALL\nINJECT",
        "ascii-call" => " \t n0call\r\n",
        _ => "N0CALL",
    };
    let mut subject = name(&[
        ("2.5.4.10", "Clacks Synthetic Test CA"),
        ("2.5.4.11", "Non-production fixtures"),
        ("1.2.840.113549.1.9.1", "test@example.invalid"),
        ("2.5.4.3", "Not the station callsign"),
    ])?;
    if variant != "common-name-only" {
        subject.extend(name(&[("1.3.6.1.4.1.12348.1.1", call)])?);
    }
    if variant == "duplicate-callsign" {
        subject.extend(name(&[("1.3.6.1.4.1.12348.1.1", "OTHER")])?);
    }
    let (first, last) = match variant {
        "reversed-coverage" => ("2029-12-31", "2000-01-01"),
        "invalid-date" => ("2000-02-30", "2029-12-31"),
        _ => ("2000-01-01", "2029-12-31"),
    };
    let mut extensions = vec![
        extension("1.3.6.1.4.1.12348.1.3", last.as_bytes())?,
        extension(
            "1.3.6.1.4.1.12348.1.4",
            if variant == "malformed-entity" {
                b"291junk"
            } else {
                b"291"
            },
        )?,
    ];
    if variant != "missing-coverage" {
        extensions.insert(0, extension("1.3.6.1.4.1.12348.1.2", first.as_bytes())?);
    }
    if variant == "duplicate-coverage" {
        extensions.push(extension("1.3.6.1.4.1.12348.1.2", b"2000-01-01")?);
    }
    if variant == "ca" {
        extensions.push(extension("2.5.29.19", &[0x30, 0x03, 0x01, 0x01, 0xff])?);
    }
    let public_key = if variant == "ec" {
        // SEC 2's P-256 generator point is a valid fixed public point (scalar 1).
        #[derive(Sequence)]
        struct EcPublic {
            /// EC algorithm and named-curve parameters.
            algorithm: AlgorithmIdentifierOwned,
            /// Uncompressed standard generator point.
            point: BitString,
        }
        Any::encode_from(&EcPublic {
            algorithm: AlgorithmIdentifierOwned {
                oid: oid("1.2.840.10045.2.1")?,
                parameters: Some(Any::encode_from(&oid("1.2.840.10045.3.1.7")?)?),
            },
            point: BitString::new(
                0,
                hex::decode(
                    "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
                )?,
            )?,
        })?
    } else {
        use der::Decode;
        Any::from_der(key.to_public_key().to_public_key_der()?.as_bytes())?
    };
    if variant == "ca" {
        subject = name(&[("2.5.4.3", "Clacks Synthetic Test Root")])?;
    }
    let algorithm = algorithm("1.2.840.113549.1.1.11")?;
    let body = Body {
        version: explicit(0, 2),
        serial: Uint::new(&hex::decode("1234567890abcdef1234567890abcdef")?)?,
        signature: algorithm.clone(),
        issuer: name(&[("2.5.4.3", "Clacks Synthetic Test Root")])?,
        validity: Validity {
            not_before: UtcTime::from_date_time(der::DateTime::new(2020, 1, 1, 0, 0, 0)?)?,
            not_after: if variant == "no-expiry" {
                Any::encode_from(&GeneralizedTime::from_date_time(der::DateTime::new(
                    9999, 12, 31, 23, 59, 59,
                )?))?
            } else {
                Any::encode_from(&UtcTime::from_date_time(der::DateTime::new(
                    2030, 6, 15, 12, 0, 0,
                )?)?)?
            },
        },
        subject,
        public_key,
        extensions: explicit(3, extensions),
    };
    let signature = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(issuer.clone())
        .try_sign_with_rng(&mut ChaCha20Rng::from_seed([202; 32]), &body.to_der()?)?
        .to_bytes()
        .to_vec();
    Ok(Certificate {
        body,
        algorithm,
        signature: BitString::new(0, signature)?,
    }
    .to_der()?)
}
