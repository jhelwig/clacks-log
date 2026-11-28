use super::{definition::SignatureDefinitions, input::prepare_input};
use crate::certificate::{CertificateMetadata, inspect_certificate};
use clacks_model::{
    ids::DestinationId,
    scalar::UtcSecond,
    service::{
        destination::LotwRecordKey,
        lotw::{LotwMappingProvenance, LotwSubmission, LotwSubmissionFields},
    },
};
use std::collections::BTreeMap;

fn sample() -> (CertificateMetadata, LotwSubmission) {
    let meta =
        inspect_certificate(include_bytes!("../../tests/fixtures/1024/station.der")).unwrap();
    let start = jiff::Timestamp::constant(1_792_672_496, 0);
    let submission: LotwSubmission = LotwSubmissionFields {
        mapping_provenance: LotwMappingProvenance {
            release: "11.34".into(),
            upstream_commit: "6ce824363e27da455c6cf6fd440b9c401945bfcb".into(),
        },
        destination: DestinationId::new(),
        signing_identity: meta.identity.clone(),
        contact_fields: [
            ("CALL", "K1ABC"),
            ("BAND", "20M"),
            ("MODE", "FT8"),
            ("FREQ", "14.074000"),
            ("QSO_DATE", "20261022"),
            ("QSO_TIME", "123456"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
        station_fields: [
            ("CQZ", "5"),
            ("GRIDSQUARE", "FN31"),
            ("ITUZ", "8"),
            ("US_STATE", "CT"),
            ("US_COUNTY", "Hartford"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
        key: LotwRecordKey {
            own_call: meta.identity.callsign.clone(),
            worked_call: clacks_model::scalar::RecordedCallsign::new("K1ABC").unwrap(),
            start: UtcSecond::from_unix_seconds(start.as_second()),
            band: "20M".into(),
            receive_band: "20M".into(),
            mode: "FT8".into(),
        },
    }
    .try_into()
    .expect("coherent frozen submission");
    (meta, submission)
}

#[test]
fn exact_input_is_literal_station_then_contact_without_separators() {
    let (meta, s) = sample();
    let defs = SignatureDefinitions::bundled().unwrap();
    let p = prepare_input(&s, &meta, &defs).unwrap();
    assert_eq!(
        p.signing_bytes,
        b"5FN318HARTFORDCT20MK1ABC14.074000FT82026-10-2212:34:56Z"
    );
    assert_eq!(p.signature_field, "SIGN_LOTW_V2.0");
    assert_eq!(p.contact.get("QSO_DATE").unwrap(), "2026-10-22");
    assert_eq!(p.contact.get("QSO_TIME").unwrap(), "12:34:56Z");
    assert_eq!(s.contact_fields().get("QSO_DATE").unwrap(), "20261022");
}

#[test]
fn definitions_select_numerically_and_reject_unsupported_or_ambiguous_data() {
    let data = include_str!("../../data/signatures-11.34.json");
    let mut json: serde_json::Value = serde_json::from_str(data).unwrap();
    json["definitions"].as_array_mut().unwrap().reverse();
    assert!(SignatureDefinitions::from_json(&json.to_string()).is_ok());
    let mut newer = json["definitions"][0].clone();
    newer["version"] = "10.0".into();
    json["definitions"].as_array_mut().unwrap().push(newer);
    assert!(SignatureDefinitions::from_json(&json.to_string()).is_err());
    json["definitions"][2]["deprecated"] = true.into();
    assert!(SignatureDefinitions::from_json(&json.to_string()).is_ok());
    let mut duplicate: serde_json::Value = serde_json::from_str(data).unwrap();
    let row = duplicate["definitions"][0].clone();
    duplicate["definitions"].as_array_mut().unwrap().push(row);
    assert!(SignatureDefinitions::from_json(&duplicate.to_string()).is_err());
    for (path, value) in [
        (
            "/definitions/0/contact/0/name",
            serde_json::json!("UNSUPPORTED"),
        ),
        ("/definitions/0/version", serde_json::json!("2.x")),
        ("/release", serde_json::json!("other")),
    ] {
        let mut invalid: serde_json::Value = serde_json::from_str(data).unwrap();
        *invalid.pointer_mut(path).unwrap() = value;
        assert!(SignatureDefinitions::from_json(&invalid.to_string()).is_err());
    }
}

#[test]
fn each_station_field_contributes_its_trimmed_uppercase_value() {
    let (meta, s) = sample();
    let defs = SignatureDefinitions::bundled().unwrap();
    let cases = [
        ("AU_STATE", "NSW"),
        ("CA_PROVINCE", "ON"),
        ("CA_US_PARK", "AA12"),
        ("CN_PROVINCE", "BJ"),
        ("CQZ", "5"),
        ("DX_US_PARK", "HP47"),
        ("FI_KUNTA", "100"),
        ("GRIDSQUARE", "fn31"),
        ("IOTA", "eu-001"),
        ("ITUZ", "8"),
        ("JA_CITY_GUN_KU", "0101"),
        ("JA_PREFECTURE", "01"),
        ("RU_OBLAST", "KA"),
        ("US_COUNTY", "Hartford"),
        ("US_PARK", "NP33"),
        ("US_STATE", "CT"),
    ];
    for (name, value) in cases {
        let mut fields = s.clone().into_fields();
        fields.station_fields = BTreeMap::from([(name.to_owned(), format!(" {value} "))]);
        let s = fields.try_into().unwrap();
        let p = prepare_input(&s, &meta, &defs).unwrap();
        assert_eq!(
            String::from_utf8(p.signing_bytes).unwrap(),
            format!(
                "{}20MK1ABC14.074000FT82026-10-2212:34:56Z",
                value.to_ascii_uppercase()
            ),
            "{name}"
        );
    }
}

#[test]
fn malformed_text_extra_fields_inconsistent_keys_and_provenance_fail() {
    let (meta, s) = sample();
    let defs = SignatureDefinitions::bundled().unwrap();
    for (name, value) in [
        ("FREQ", "1\n2"),
        ("FREQ", "14.é"),
        ("FREQ", "\0"),
        ("UNKNOWN", "X"),
    ] {
        let mut fields = s.clone().into_fields();
        fields.contact_fields.insert(name.into(), value.into());
        let changed = fields.try_into().unwrap();
        assert!(prepare_input(&changed, &meta, &defs).is_err(), "{name}");
    }
    let mut fields = s.clone().into_fields();
    fields.mapping_provenance.release = "other".into();
    let changed = fields.try_into().unwrap();
    assert!(prepare_input(&changed, &meta, &defs).is_err());
}
