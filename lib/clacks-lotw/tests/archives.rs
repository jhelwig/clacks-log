use crate::support;

use clacks_lotw::certificate::import_pkcs12;

#[test]
fn imports_modern_and_legacy_layouts_with_actual_key_matching() -> anyhow::Result<()> {
    for bits in [1024, 2048, 4096] {
        for name in ["normal.p12", "legacy.p12"] {
            let imported = import_pkcs12(
                &support::fixture(&format!("{bits}/{name}")),
                "fixture-password",
            )?;
            assert_eq!(imported.metadata().identity.callsign.as_str(), "N0CALL");
            assert_eq!(
                imported.certificate_der(),
                support::fixture(&format!("{bits}/station.der"))
            );
        }
    }
    Ok(())
}

#[test]
fn requires_integrity_matching_and_unambiguous_station_material() {
    for name in [
        "no-mac.p12",
        "mismatch.p12",
        "ambiguous.p12",
        "no-certificate.p12",
        "bad-metadata.p12",
    ] {
        assert!(
            import_pkcs12(&support::fixture(name), "fixture-password").is_err(),
            "{name}"
        );
    }
    assert!(import_pkcs12(&support::fixture("1024/normal.p12"), "wrong").is_err());
    let mut corrupted = support::fixture("1024/normal.p12");
    if let Some(last) = corrupted.last_mut() {
        *last ^= 1;
    }
    assert!(import_pkcs12(&corrupted, "fixture-password").is_err());
}

#[test]
fn supports_empty_bmp_passwords_and_duplicate_bags_without_alias_trust() -> anyhow::Result<()> {
    for (name, password) in [
        ("empty.p12", ""),
        ("bmp.p12", "test-café"),
        ("duplicate-bags.p12", "fixture-password"),
        ("misleading-alias.p12", "fixture-password"),
    ] {
        let imported = import_pkcs12(&support::fixture(name), password)?;
        assert_eq!(imported.metadata().identity.callsign.as_str(), "N0CALL");
    }
    assert!(import_pkcs12(&support::fixture("1024/normal.p12"), "test-😀").is_err());
    Ok(())
}

#[test]
fn rejects_excessive_work_and_malformed_framing_without_unbounded_fallback() {
    for name in [
        "mac-iterations-1000001.p12",
        "mac-iterations-0.p12",
        "mac-iterations--1.p12",
        "modern-kdf-excessive.p12",
        "legacy-kdf-excessive.p12",
        "scrypt-excessive-memory.p12",
        "scrypt-excessive-parallel.p12",
    ] {
        assert!(
            import_pkcs12(&support::fixture(name), "fixture-password").is_err(),
            "{name}"
        );
    }
    let mut truncated = support::fixture("1024/normal.p12");
    truncated.truncate(20);
    assert!(import_pkcs12(&truncated, "fixture-password").is_err());
}

#[test]
fn bounds_private_integer_encodings_before_private_validation() {
    use clacks_lotw::error::ImportError;
    for name in ["oversized-modulus.p12", "oversized-private_exponent.p12"] {
        let result = import_pkcs12(&support::fixture(name), "fixture-password");
        let error = result.err().expect("oversized key rejected");
        assert!(
            matches!(
                error.downcast_ref::<ImportError>(),
                Some(ImportError::KeyTooLarge)
            ),
            "{error:#}"
        );
    }
}

#[test]
fn integrity_failure_preserves_the_archive_error_for_diagnostics() {
    let error = import_pkcs12(&support::fixture("1024/normal.p12"), "wrong")
        .err()
        .expect("integrity failure");

    assert!(error.downcast_ref::<p12_keystore::error::Error>().is_some());
}

#[test]
fn validates_raw_certificate_callsigns_before_native_normalization() -> anyhow::Result<()> {
    use clacks_lotw::error::CertificateError;
    for name in [
        "unicode-expands.p12",
        "unicode-remains.p12",
        "control-call.p12",
    ] {
        let result = import_pkcs12(&support::fixture(name), "fixture-password");
        let error = result.err().expect("unsupported raw identity must fail");
        assert_eq!(
            error.downcast_ref::<CertificateError>().unwrap().field,
            "AROcallsign"
        );
    }
    let valid = import_pkcs12(&support::fixture("ascii-call.p12"), "fixture-password")?;
    assert_eq!(valid.metadata().identity.callsign.as_str(), "N0CALL");
    Ok(())
}

#[test]
fn rejects_nonpositive_iterations_even_with_a_matching_one_round_mac() -> anyhow::Result<()> {
    let valid = import_pkcs12(&support::fixture("mac-one-round-1.p12"), "fixture-password")?;
    assert_eq!(valid.metadata().identity.callsign.as_str(), "N0CALL");
    for n in [0, -1] {
        let error = import_pkcs12(
            &support::fixture(&format!("mac-one-round-{n}.p12")),
            "fixture-password",
        )
        .err()
        .expect("nonpositive rounds must fail");
        assert!(matches!(
            error.downcast_ref::<clacks_lotw::error::ImportError>(),
            Some(clacks_lotw::error::ImportError::InvalidMacIterations)
        ));
    }
    Ok(())
}

#[test]
fn exercises_archive_metadata_chain_encryption_and_password_boundaries() -> anyhow::Result<()> {
    for name in [
        "duplicate-callsign.p12",
        "duplicate-coverage.p12",
        "unsupported-encryption.p12",
    ] {
        assert!(
            import_pkcs12(&support::fixture(name), "fixture-password").is_err(),
            "{name}"
        );
    }
    for (name, password) in [
        ("supporting-ca.p12", "fixture-password"),
        ("encrypted-empty.p12", ""),
        ("encrypted-bmp.p12", "test-café"),
    ] {
        let c = import_pkcs12(&support::fixture(name), password)?;
        assert_eq!(c.certificate_der(), support::fixture("1024/station.der"));
    }
    Ok(())
}

#[test]
fn imports_source_layout_and_reordered_modern_and_legacy_exports() -> anyhow::Result<()> {
    use der::{Decode, asn1::OctetString};
    use pkcs12::{Pfx, cms::content_info::ContentInfo};
    for (file, mac_oid, safe_types) in [
        (
            "1024/normal.p12",
            "2.16.840.1.101.3.4.2.1",
            ["1.2.840.113549.1.7.6", "1.2.840.113549.1.7.1"],
        ),
        (
            "normal-key-first.p12",
            "2.16.840.1.101.3.4.2.1",
            ["1.2.840.113549.1.7.1", "1.2.840.113549.1.7.6"],
        ),
        (
            "1024/legacy.p12",
            "1.3.14.3.2.26",
            ["1.2.840.113549.1.7.6", "1.2.840.113549.1.7.1"],
        ),
        (
            "legacy-key-first.p12",
            "1.3.14.3.2.26",
            ["1.2.840.113549.1.7.1", "1.2.840.113549.1.7.6"],
        ),
    ] {
        let bytes = support::fixture(file);
        let pfx = Pfx::from_der(&bytes)?;
        assert_eq!(
            pfx.mac_data.unwrap().mac.algorithm.oid.to_string(),
            mac_oid,
            "{file}"
        );
        let contents = pfx.auth_safe.content.decode_as::<OctetString>()?;
        let safes = Vec::<ContentInfo>::from_der(contents.as_bytes())?;
        assert_eq!(
            safes
                .iter()
                .map(|safe| safe.content_type.to_string())
                .collect::<Vec<_>>(),
            safe_types,
            "{file}"
        );
        let imported = import_pkcs12(&bytes, "fixture-password")?;
        assert_eq!(
            imported.certificate_der(),
            support::fixture("1024/station.der")
        );
        assert!(import_pkcs12(&bytes, "wrong").is_err());
        let mut corrupt = bytes;
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(import_pkcs12(&corrupt, "fixture-password").is_err());
    }
    Ok(())
}

#[test]
fn characterizes_nonpositive_legacy_encryption_counts_with_valid_integrity() -> anyhow::Result<()> {
    for count in [1, 0, -1] {
        let bytes = support::fixture(&format!("legacy-one-round-{count}.p12"));
        let (mac_count, encryption_counts) = legacy_derivation_counts(&bytes)?;
        assert_eq!(mac_count, 2048);
        assert_eq!(encryption_counts, [count, count]);
        let imported = import_pkcs12(&bytes, "fixture-password")?;
        assert_eq!(
            imported.certificate_der(),
            support::fixture("1024/station.der")
        );
        assert!(import_pkcs12(&bytes, "wrong").is_err());
    }
    Ok(())
}

#[test]
fn rejects_bounded_but_invalid_private_components_during_key_decoding() {
    let error = import_pkcs12(
        &support::fixture("invalid-private-exponent.p12"),
        "fixture-password",
    )
    .err()
    .expect("invalid private exponent must fail");
    assert!(
        error.downcast_ref::<rsa::pkcs8::Error>().is_some(),
        "{error:#}"
    );
}

/// Inspect both legacy derivation declarations independently of archive decryption.
fn legacy_derivation_counts(bytes: &[u8]) -> anyhow::Result<(i32, Vec<i32>)> {
    use der::{
        Decode,
        asn1::{ContextSpecific, OctetString},
    };
    use pkcs12::{
        Pfx,
        cms::{content_info::ContentInfo, encrypted_data::EncryptedData},
        pbe_params::{EncryptedPrivateKeyInfo, Pkcs12PbeParams},
        safe_bag::SafeContents,
    };
    let pfx = Pfx::from_der(bytes)?;
    let contents = pfx.auth_safe.content.decode_as::<OctetString>()?;
    let safes = Vec::<ContentInfo>::from_der(contents.as_bytes())?;
    let mut counts = Vec::new();
    for safe in safes {
        let algorithm = match safe.content_type.to_string().as_str() {
            "1.2.840.113549.1.7.6" => {
                safe.content
                    .decode_as::<EncryptedData>()?
                    .enc_content_info
                    .content_enc_alg
            }
            "1.2.840.113549.1.7.1" => {
                let data = safe.content.decode_as::<OctetString>()?;
                let bags = SafeContents::from_der(data.as_bytes())?;
                assert_eq!(bags.len(), 1);
                ContextSpecific::<EncryptedPrivateKeyInfo>::from_der(
                    &bags.first().unwrap().bag_value,
                )?
                .value
                .encryption_algorithm
            }
            _ => anyhow::bail!("unexpected fixture safe"),
        };
        let params = algorithm
            .parameters
            .unwrap()
            .decode_as::<Pkcs12PbeParams>()?;
        counts.push(params.iterations);
    }
    Ok((pfx.mac_data.unwrap().iterations, counts))
}
