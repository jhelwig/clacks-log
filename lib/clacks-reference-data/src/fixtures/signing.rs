//! Fixed-seed synthetic keys, signatures and independent public RSA checks.

use anyhow::{Context, ensure};
use num_bigint::BigUint;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rsa::{
    RsaPrivateKey, RsaPublicKey,
    signature::{RandomizedSigner, SignatureEncoding},
    traits::PublicKeyParts,
};
use sha1::Digest;

/// Hand-authored signing input for the representative complete wire fixture.
pub(super) const INPUT: &[u8] = b"5FN318HARTFORDCT20MK1ABC14.074000FT82026-10-2212:34:56Z";

/// Construct a deterministic, public test key; never use this seed for real keys.
pub(super) fn key(bits: usize, seed: u8) -> anyhow::Result<RsaPrivateKey> {
    Ok(RsaPrivateKey::new(
        &mut ChaCha20Rng::from_seed([seed; 32]),
        bits,
    )?)
}

/// Sign an explicit fixture message without calling the production signing path.
pub(super) fn sign(key: &RsaPrivateKey, input: &[u8]) -> anyhow::Result<Vec<u8>> {
    let signer = rsa::pkcs1v15::SigningKey::<sha1::Sha1>::new(key.clone());
    let signature = signer
        .try_sign_with_rng(&mut ChaCha20Rng::from_seed([201; 32]), input)?
        .to_bytes()
        .to_vec();
    verify(&key.to_public_key(), input, &signature)?;
    Ok(signature)
}

/// Verify raw RSA exponentiation and literal PKCS#1/SHA-1 padding independently.
///
/// Uses `num-bigint`, rather than RSA's `num-bigint-dig` backend, and does not
/// call RSA's verification API. The expected DigestInfo follows RFC 8017 B.1.
pub(super) fn verify(key: &RsaPublicKey, input: &[u8], signature: &[u8]) -> anyhow::Result<()> {
    let modulus = BigUint::from_bytes_be(&key.n().to_bytes_be());
    let exponent = BigUint::from_bytes_be(&key.e().to_bytes_be());
    let value = BigUint::from_bytes_be(signature);
    ensure!(
        value < modulus && signature.len() == key.size(),
        "invalid fixture RSA signature width/range"
    );
    let recovered = value.modpow(&exponent, &modulus).to_bytes_be();
    let zeros = key
        .size()
        .checked_sub(recovered.len())
        .context("RSA result exceeds modulus width")?;
    let mut actual = vec![0; zeros];
    actual.extend_from_slice(&recovered);
    let mut digest_info = hex::decode("3021300906052b0e03021a05000414")?;
    digest_info.extend_from_slice(&sha1::Sha1::digest(input));
    let padding = key
        .size()
        .checked_sub(digest_info.len() + 3)
        .context("RSA key too small for SHA-1 padding")?;
    ensure!(
        padding >= 8,
        "RSA padding must contain at least eight bytes"
    );
    let mut expected = vec![0, 1];
    expected.extend(std::iter::repeat_n(0xff, padding));
    expected.push(0);
    expected.extend_from_slice(&digest_info);
    ensure!(
        actual == expected,
        "fixture RSA signature does not match literal PKCS#1/SHA-1 encoding"
    );
    Ok(())
}

/// Wrap base64 at the protocol's literal 64-column boundary with a final newline.
fn base64_lines(bytes: &[u8]) -> Vec<u8> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let encoded = STANDARD.encode(bytes);
    let mut result = Vec::new();
    for line in encoded.as_bytes().chunks(64) {
        result.extend_from_slice(line);
        result.push(b'\n');
    }
    result
}

/// Assemble a complete wire expectation from literal fields and fixture bytes.
///
/// Field order/framing is authored here and never obtained from Clacks output.
pub(super) fn wire(certificate: &[u8], signature: &[u8]) -> Vec<u8> {
    let certificate = base64_lines(certificate);
    let signature = base64_lines(signature);
    let mut out = b"<TQSL_IDENT:57>Clacks Log test Compatibility: TQSL V2.8.6 Config: V11.34\n<Rec_Type:5>tCERT\n<CERT_UID:1>1\n".to_vec();
    out.extend_from_slice(format!("<CERTIFICATE:{}>", certificate.len()).as_bytes());
    out.extend_from_slice(&certificate);
    out.extend_from_slice(b"<eor>\n<Rec_Type:8>tSTATION\n<STATION_UID:1>1\n<CERT_UID:1>1\n<CALL:6>N0CALL\n<CQZ:1>5\n<DXCC:3>291\n<GRIDSQUARE:4>FN31\n<ITUZ:1>8\n<US_COUNTY:8>Hartford\n<US_STATE:2>CT\n<eor>\n<Rec_Type:8>tCONTACT\n<STATION_UID:1>1\n<BAND:3>20M\n<CALL:5>K1ABC\n<FREQ:9>14.074000\n<MODE:3>FT8\n<QSO_DATE:10>2026-10-22\n<QSO_TIME:9>12:34:56Z\n");
    out.extend_from_slice(format!("<SIGN_LOTW_V2.0:{}:6>", signature.len()).as_bytes());
    out.extend_from_slice(&signature);
    out.extend_from_slice(b"<SIGNDATA:55>");
    out.extend_from_slice(INPUT);
    out.extend_from_slice(b"\n<eor>\n");
    out
}
