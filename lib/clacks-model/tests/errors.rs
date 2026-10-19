use clacks_model::*;
use std::error::Error;

#[test]
fn error_messages_preserve_diagnostic_context() {
    let value_cases = [
        (
            ValueError::InvalidRecognitionCatalog {
                reason: "duplicate".into(),
            },
            "invalid recognition catalog: duplicate",
        ),
        (
            ValueError::InvalidCoordinate {
                axis: "latitude".into(),
                value: "91".into(),
            },
            "latitude outside geographic range: 91",
        ),
        (
            ValueError::InvalidLocator { value: "ZZ".into() },
            "invalid Maidenhead locator: ZZ",
        ),
        (
            ValueError::InvalidGridLength { length: 3 },
            "invalid locator length: 3",
        ),
        (
            ValueError::MalformedDecimal {
                value: "oops".into(),
            },
            "invalid decimal degrees: oops",
        ),
        (
            ValueError::InvalidZone {
                system: "CQ".into(),
                value: 0,
            },
            "invalid CQ zone: 0",
        ),
        (
            ValueError::EmptyText {
                field: "callsign".into(),
            },
            "callsign cannot be blank",
        ),
        (
            ValueError::ZeroQuantity {
                unit: "hertz".into(),
            },
            "hertz must be positive",
        ),
        (
            ValueError::InvalidId {
                value: "oops".into(),
            },
            "invalid ID: oops",
        ),
        (
            ValueError::InvalidTimestamp {
                nanoseconds: 1_000_000_000,
            },
            "nanoseconds outside 0..1,000,000,000: 1000000000",
        ),
    ];
    for (error, expected) in value_cases {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none());
    }

    let equipment = EquipmentConfigurationId::from_u128(1);
    let other_equipment = EquipmentConfigurationId::from_u128(2);
    let record_cases = [
        (
            RecordError::MissingBandAndFrequency,
            "a frequency or band is required",
        ),
        (
            RecordError::EndBeforeStart {
                start: UtcSecond::from_unix_seconds(20),
                end: UtcSecond::from_unix_seconds(10),
            },
            "end 10 precedes start 20",
        ),
        (
            RecordError::EmptyOptionalAggregate {
                field: "location".into(),
            },
            "empty location must be normalized to absence before saving",
        ),
        (
            RecordError::InvalidOptionalText {
                field: "comment".into(),
            },
            "blank optional comment must be absent",
        ),
        (
            RecordError::MissingEquipment { id: equipment },
            "equipment configuration 00000000000000000000000001 is unavailable",
        ),
        (
            RecordError::EquipmentIdMismatch {
                expected: equipment,
                supplied: other_equipment,
            },
            "expected equipment 00000000000000000000000001, supplied 00000000000000000000000002",
        ),
    ];
    for (error, expected) in record_cases {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none());
    }

    let group = GroupId::from_u128(1);
    let definition_cases = [
        (
            DefinitionError::DuplicateGroupId { id: group },
            "competing definitions for group 00000000000000000000000001",
        ),
        (
            DefinitionError::MissingIdentifier { group_id: group },
            "group 00000000000000000000000001 requires an identifier",
        ),
        (
            DefinitionError::InvalidIdentifier {
                group_id: group,
                value: "bad\nvalue".into(),
            },
            "identifier \"bad\\nvalue\" does not match group 00000000000000000000000001",
        ),
        (
            DefinitionError::InvalidDefinition {
                group_id: group,
                path: "mappings[0]".into(),
                reason: "missing field".into(),
            },
            "group 00000000000000000000000001 at mappings[0]: missing field",
        ),
    ];
    for (error, expected) in definition_cases {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none());
    }

    let qso = QsoId::from_u128(1);
    let export_cases = [
        (
            ExportError::UnknownProfile {
                id: OutputProfileId::from_u128(1),
            },
            "unknown output profile 00000000000000000000000001",
        ),
        (
            ExportError::UnknownTarget { id: group },
            "unknown target group 00000000000000000000000001",
        ),
        (
            ExportError::InvalidResolution {
                qso_id: qso,
                reason: "wrong group".into(),
            },
            "invalid export choice for 00000000000000000000000001: wrong group",
        ),
        (
            ExportError::ConflictingBindings {
                group_id: group,
                field: "CONTEST_ID".into(),
            },
            "group 00000000000000000000000001 conflicts on output field CONTEST_ID",
        ),
        (
            ExportError::NotReady {
                qso_id: qso,
                reason: "missing partition".into(),
            },
            "QSO 00000000000000000000000001 is not ready for output: missing partition",
        ),
    ];
    for (error, expected) in export_cases {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none());
    }
}

#[test]
fn record_errors_expose_wrapped_values_as_their_source() {
    let value = ValueError::EmptyText {
        field: "callsign".into(),
    };
    let error = RecordError::from(value.clone());
    assert_eq!(error.to_string(), "callsign cannot be blank");
    let source = error.source().expect("wrapped value must be available");
    assert_eq!(source.downcast_ref::<ValueError>(), Some(&value));
    assert!(source.source().is_none());
}

#[test]
fn export_errors_expose_wrapped_definitions_as_their_source() {
    let definition = DefinitionError::InvalidDefinition {
        group_id: GroupId::from_u128(1),
        path: "mapping".into(),
        reason: "invalid regex".into(),
    };
    let error = ExportError::from(definition.clone());
    assert_eq!(
        error.to_string(),
        "group 00000000000000000000000001 at mapping: invalid regex"
    );
    let source = error
        .source()
        .expect("wrapped definition must be available");
    assert_eq!(source.downcast_ref::<DefinitionError>(), Some(&definition));
    assert!(source.source().is_none());
}
