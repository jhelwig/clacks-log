mod support;

use clacks_model::contact::RecordError;
use clacks_model::error::ValueError;
use clacks_model::export::ExportContext;
use clacks_model::export::evaluate::plan_reference_export;
use clacks_model::export::select::{ExportError, select_mappings};
use clacks_model::group::{DefinitionError, DefinitionSet};
use clacks_model::ids::{EquipmentConfigurationId, GroupId, OutputProfileId, QsoId};
use clacks_model::scalar::UtcSecond;
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
fn export_operations_propagate_definition_errors_without_forwarding_variants() {
    let mut group = support::definitions().iter().next().unwrap().clone();
    group.identifier.syntax = Some("[".into());
    let mut qso = support::qso(QsoId::from_u128(1));
    qso.own_station
        .references
        .insert(support::reference(group.id, Some("test")));
    let definitions = DefinitionSet::new(vec![group]).unwrap();
    let error = select_mappings(
        &qso,
        &ExportContext {
            output_profile_id: OutputProfileId::from_u128(1),
            target_group_id: None,
        },
        &definitions,
        &support::profiles(),
        None,
    )
    .unwrap_err();
    assert!(
        error.downcast_ref::<DefinitionError>().is_some(),
        "original definition error must remain directly downcastable"
    );
}

#[test]
fn record_operations_return_downcastable_validation_errors() {
    let mut qso = support::qso(QsoId::from_u128(1));
    qso.end = Some(UtcSecond::from_unix_seconds(999));
    let error = qso.validate().unwrap_err();
    assert!(matches!(
        error.downcast_ref::<RecordError>(),
        Some(RecordError::EndBeforeStart { .. })
    ));
    assert_eq!(error.to_string(), "end 999 precedes start 1000");
    assert!(error.root_cause().is::<RecordError>());
}

#[test]
fn export_readiness_context_retains_the_validation_cause() {
    let mut qso = support::qso(QsoId::from_u128(1));
    qso.end = Some(UtcSecond::from_unix_seconds(999));
    let error = plan_reference_export(
        &[qso],
        &ExportContext {
            output_profile_id: OutputProfileId::from_u128(1),
            target_group_id: None,
        },
        &support::definitions(),
        &support::profiles(),
        &std::collections::BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "QSO 00000000000000000000000001 is not ready for output: end 999 precedes start 1000"
    );
    assert!(matches!(
        error.downcast_ref::<ExportError>(),
        Some(ExportError::NotReady { .. })
    ));
    assert!(matches!(
        error.downcast_ref::<RecordError>(),
        Some(RecordError::EndBeforeStart { .. })
    ));
    assert_eq!(error.chain().count(), 2);
}
