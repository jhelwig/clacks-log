use crate::support;

use base64::{Engine, engine::general_purpose::STANDARD};
use clacks_lotw::{
    certificate::{eligibility::CredentialState, import_pkcs12},
    signing::{BatchItem, ProducerInformation, definition::SignatureDefinitions, write_batch},
};
use std::io::ErrorKind;

fn producer() -> ProducerInformation<'static> {
    ProducerInformation {
        application: "Clacks Log",
        version: "test",
    }
}

#[test]
fn shares_certificate_and_station_records_without_changing_membership() -> anyhow::Result<()> {
    let c = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
    let same = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
    let other = import_pkcs12(&support::fixture("2048/normal.p12"), "fixture-password")?;
    let a = support::submission();
    let mut fields = a.clone().into_fields();
    fields
        .station_fields
        .insert("GRIDSQUARE".into(), "FN32".into());
    let b = fields.try_into()?;
    let defs = SignatureDefinitions::bundled()?;
    for (second, credential, want) in [
        (&a, &same, (1, 1, 2)),
        (&b, &same, (1, 2, 2)),
        (&a, &other, (2, 2, 2)),
    ] {
        let items = [
            BatchItem {
                submission: &a,
                credential: &c,
                state: CredentialState::Available,
            },
            BatchItem {
                submission: second,
                credential,
                state: CredentialState::Available,
            },
        ];
        let mut bytes = Vec::new();
        let summary = write_batch(&mut bytes, &items, &defs, &producer(), support::now())?;
        assert_eq!(
            (summary.certificates, summary.stations, summary.contacts),
            want
        );
        assert_eq!(summary.bytes_written, bytes.len() as u64);
        let fields = support::fields(&bytes);
        let certs: Vec<_> = fields.iter().filter(|f| f.name == "CERTIFICATE").collect();
        assert_eq!(
            STANDARD.decode(
                certs[0]
                    .bytes
                    .iter()
                    .copied()
                    .filter(|b| !b.is_ascii_whitespace())
                    .collect::<Vec<_>>()
            )?,
            c.certificate_der()
        );
        assert_eq!(
            fields
                .iter()
                .filter(|f| f.name == "REC_TYPE" && f.bytes == b"tCONTACT")
                .count(),
            2
        );
        let records: Vec<_> = fields.split(|f| f.name == "REC_TYPE").skip(1).collect();
        let value = |record: &[support::Field], name: &str| {
            record
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.bytes.clone())
                .unwrap()
        };
        let station_records: Vec<_> = records
            .iter()
            .filter(|r| r.iter().any(|f| f.name == "DXCC"))
            .collect();
        let contact_records: Vec<_> = records
            .iter()
            .filter(|r| r.iter().any(|f| f.name == "SIGNDATA"))
            .collect();
        let expected_second = if want.1 == 1 {
            b"1".to_vec()
        } else {
            b"2".to_vec()
        };
        assert_eq!(
            contact_records
                .iter()
                .map(|r| value(r, "STATION_UID"))
                .collect::<Vec<_>>(),
            [b"1".to_vec(), expected_second]
        );
        for (index, station) in station_records.iter().enumerate() {
            assert_eq!(
                value(station, "STATION_UID"),
                (index + 1).to_string().as_bytes()
            );
            let expected_certificate = if want.0 == 2 { index + 1 } else { 1 };
            assert_eq!(
                value(station, "CERT_UID"),
                expected_certificate.to_string().as_bytes()
            );
            if want.0 == 1 && want.1 == 2 {
                assert_eq!(
                    value(station, "GRIDSQUARE"),
                    if index == 0 { b"FN31" } else { b"FN32" }
                );
            }
        }
        for (index, certificate) in certs.iter().enumerate() {
            let decoded = STANDARD.decode(
                certificate
                    .bytes
                    .iter()
                    .copied()
                    .filter(|b| !b.is_ascii_whitespace())
                    .collect::<Vec<_>>(),
            )?;
            assert_eq!(
                decoded,
                if index == 0 {
                    c.certificate_der()
                } else {
                    other.certificate_der()
                }
            );
        }
    }
    Ok(())
}

#[test]
fn signatures_match_independent_known_answers_and_exact_wire_values() -> anyhow::Result<()> {
    use rsa::{pkcs8::DecodePublicKey, signature::Verifier};
    let a = support::submission();
    let defs = SignatureDefinitions::bundled()?;
    for bits in [1024, 2048, 4096] {
        let c = import_pkcs12(
            &support::fixture(&format!("{bits}/normal.p12")),
            "fixture-password",
        )?;
        let items = [BatchItem {
            submission: &a,
            credential: &c,
            state: CredentialState::Available,
        }];
        let mut bytes = Vec::new();
        write_batch(&mut bytes, &items, &defs, &producer(), support::now())?;
        let fields = support::fields(&bytes);
        let signature = fields.iter().find(|f| f.name == "SIGN_LOTW_V2.0").unwrap();
        assert_eq!(signature.kind.as_deref(), Some("6"));
        let decoded = STANDARD.decode(
            signature
                .bytes
                .iter()
                .copied()
                .filter(|b| !b.is_ascii_whitespace())
                .collect::<Vec<_>>(),
        )?;
        assert_eq!(
            decoded,
            support::fixture(&format!("{bits}/known-signature.bin"))
        );
        let data = fields.iter().find(|f| f.name == "SIGNDATA").unwrap();
        assert_eq!(
            data.bytes,
            b"5FN318HARTFORDCT20MK1ABC14.074000FT82026-10-2212:34:56Z"
        );
        let public = rsa::RsaPublicKey::from_public_key_pem(std::str::from_utf8(
            &support::fixture(&format!("{bits}/public.pem")),
        )?)?;
        let verifier = rsa::pkcs1v15::VerifyingKey::<sha1::Sha1>::new(public);
        let sig = rsa::pkcs1v15::Signature::try_from(decoded.as_slice())?;
        verifier.verify(&data.bytes, &sig)?;
        for (name, want) in [
            ("QSO_DATE", "2026-10-22"),
            ("QSO_TIME", "12:34:56Z"),
            ("FREQ", "14.074000"),
            ("CALL", "N0CALL"),
            ("DXCC", "291"),
        ] {
            assert!(
                fields
                    .iter()
                    .any(|f| f.name == name && f.bytes == want.as_bytes())
            );
        }
        assert!(
            fields
                .iter()
                .find(|f| f.name == "TQSL_IDENT")
                .unwrap()
                .bytes
                .starts_with(b"Clacks Log test")
        );
    }
    Ok(())
}

#[test]
fn writer_failure_identifies_the_batch_item_and_preserves_the_io_cause() -> anyhow::Result<()> {
    let a = support::submission();
    let defs = SignatureDefinitions::bundled()?;
    let c = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
    let items = [
        BatchItem {
            submission: &a,
            credential: &c,
            state: CredentialState::Available,
        },
        BatchItem {
            submission: &a,
            credential: &c,
            state: CredentialState::Available,
        },
    ];
    let mut one = Vec::new();
    write_batch(&mut one, &items[..1], &defs, &producer(), support::now())?;
    let mut failed = support::LimitedWriter {
        bytes: Vec::new(),
        chunk: usize::MAX,
        fail_at: Some(one.len() + 10),
    };
    let error = write_batch(&mut failed, &items, &defs, &producer(), support::now()).unwrap_err();
    assert_eq!(error.to_string(), "write batch item 1");
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        ErrorKind::BrokenPipe
    );
    Ok(())
}

#[test]
fn rejects_empty_or_ineligible_batches_before_emitting_bytes() -> anyhow::Result<()> {
    let a = support::submission();
    let defs = SignatureDefinitions::bundled()?;
    let c = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
    let mut bytes = Vec::new();
    assert!(write_batch(&mut bytes, &[], &defs, &producer(), support::now()).is_err());
    let items = [
        BatchItem {
            submission: &a,
            credential: &c,
            state: CredentialState::Available,
        },
        BatchItem {
            submission: &a,
            credential: &c,
            state: CredentialState::Superseded,
        },
    ];
    assert!(write_batch(&mut bytes, &items, &defs, &producer(), support::now()).is_err());
    assert!(bytes.is_empty());
    Ok(())
}

#[test]
fn producer_delimiters_remain_length_framed_data() -> anyhow::Result<()> {
    let a = support::submission();
    let c = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
    let mut bytes = Vec::new();
    write_batch(
        &mut bytes,
        &[BatchItem {
            submission: &a,
            credential: &c,
            state: CredentialState::Available,
        }],
        &SignatureDefinitions::bundled()?,
        &ProducerInformation {
            application: "Clacks <eor><Rec_Type:8>tCONTACT",
            version: "test",
        },
        support::now(),
    )?;
    let fields = support::fields(&bytes);
    assert_eq!(fields.iter().filter(|f| f.name == "REC_TYPE").count(), 3);
    assert!(fields.first().unwrap().bytes.starts_with(b"Clacks <eor>"));
    Ok(())
}

#[test]
fn matches_a_complete_independently_authored_wire_expectation() -> anyhow::Result<()> {
    let a = support::submission();
    let c = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
    let mut bytes = Vec::new();
    write_batch(
        &mut bytes,
        &[BatchItem {
            submission: &a,
            credential: &c,
            state: CredentialState::Available,
        }],
        &SignatureDefinitions::bundled()?,
        &producer(),
        support::now(),
    )?;
    assert_eq!(bytes, support::fixture("single-contact.gabbi"));
    Ok(())
}
