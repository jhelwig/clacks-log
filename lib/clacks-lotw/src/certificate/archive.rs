//! Checked archive decoding and actual RSA key matching.

use super::{SigningCredential, inspect_certificate};
use crate::error::ImportError;
use anyhow::{Context, ensure};
use der::Decode;
use p12_keystore::Pkcs12Archive;
use pkcs12::pfx::Pfx;
use rsa::{pkcs8::DecodePrivateKey, traits::PublicKeyParts};
use std::collections::BTreeSet;

/// Reject unbounded integer encodings before private-key construction/validation.
fn private_key(der: &[u8]) -> anyhow::Result<rsa::PrivateKey> {
    let info =
        rsa::pkcs8::PrivateKeyInfoRef::try_from(der).context("decode private key envelope")?;
    let raw = rsa::pkcs1::RsaPrivateKeyRef::try_from(info.private_key.as_bytes())
        .context("decode RSA private components")?;
    for value in [
        raw.modulus,
        raw.public_exponent,
        raw.private_exponent,
        raw.prime1,
        raw.prime2,
        raw.exponent1,
        raw.exponent2,
        raw.coefficient,
    ] {
        ensure!(value.as_bytes().len() <= 512, ImportError::KeyTooLarge);
    }
    rsa::PrivateKey::from_pkcs8_der(der).context("validate RSA private key")
}

/// Decode an integrity-checked archive into one opaque station credential.
pub(super) fn import(bytes: &[u8], password: &str) -> anyhow::Result<SigningCredential> {
    ensure!(
        password.chars().all(|c| u32::from(c) <= 0xffff),
        ImportError::UnsupportedPassword
    );
    let framing = Pfx::from_der(bytes).context("decode certificate archive framing")?;
    let Some(mac) = framing.mac_data else {
        tracing::debug!(event = "certificate.import.rejected", reason = ?ImportError::MissingMac);
        return Err(ImportError::MissingMac.into());
    };
    if mac.iterations <= 0 {
        tracing::debug!(event = "certificate.import.rejected", reason = ?ImportError::InvalidMacIterations);
        return Err(ImportError::InvalidMacIterations.into());
    }
    let archive = Pkcs12Archive::from_pkcs12(bytes, password)
        .context("verify archive integrity and decrypt material")?;
    let mut seen = BTreeSet::new();
    let mut certificates = Vec::new();
    for bag in &archive.certs {
        let der = bag.cert.as_der();
        if !seen.insert(der) {
            continue;
        }
        let (_, cert) = x509_parser::parse_x509_certificate(der)
            .map_err(|e| e.to_owned())
            .context("decode archive certificate")?;
        if cert.basic_constraints()?.is_some_and(|b| b.value.ca) {
            continue;
        }
        let metadata = inspect_certificate(der)?;
        certificates.push((der, metadata));
    }
    let keys: BTreeSet<_> = archive.keys.iter().map(|bag| bag.key.as_der()).collect();
    ensure!(
        !keys.is_empty() && !certificates.is_empty(),
        ImportError::MissingMaterial
    );
    ensure!(
        keys.len() == 1 && certificates.len() == 1,
        ImportError::AmbiguousMaterial
    );
    let (der, metadata) = certificates
        .into_iter()
        .next()
        .context("station selection unexpectedly empty")?;
    let encoded_key = keys
        .into_iter()
        .next()
        .context("key selection unexpectedly empty")?;
    let key = private_key(encoded_key)?;
    let (_, certificate) = x509_parser::parse_x509_certificate(der).map_err(|e| e.to_owned())?;
    let public = rsa::pkcs1::RsaPublicKeyRef::try_from(
        certificate.public_key().subject_public_key.data.as_ref(),
    )
    .context("decode station RSA public key")?;
    ensure!(
        public.modulus.as_bytes().len() <= 512 && public.public_exponent.as_bytes().len() <= 512,
        ImportError::KeyTooLarge
    );
    ensure!(
        key.n().to_be_bytes_trimmed_vartime().as_ref() == public.modulus.as_bytes()
            && key.e().to_be_bytes_trimmed_vartime().as_ref() == public.public_exponent.as_bytes(),
        ImportError::KeyMismatch
    );
    tracing::debug!(
        event = "certificate.import.checked",
        certificate_count = 1,
        key_count = 1
    );
    Ok(SigningCredential {
        metadata,
        certificate_der: der.to_vec(),
        signing_key: rsa::pkcs1v15::SigningKey::new(key),
    })
}
