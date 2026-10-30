use crate::support;

use clacks_model::contact::RecordError;
use clacks_model::group::DefinitionError;
use clacks_model::ids::QsoId;
use clacks_model::scalar::UtcSecond;

#[test]
fn definition_input_returns_downcastable_configuration_error() {
    let mut fields = support::definitions()
        .iter()
        .next()
        .unwrap()
        .clone()
        .into_fields();
    fields.identifier.syntax = Some("[".into());
    let error: anyhow::Error = clacks_model::group::GroupDefinition::try_from(fields)
        .unwrap_err()
        .into();
    assert!(error.downcast_ref::<DefinitionError>().is_some());
}

#[test]
fn record_operations_return_downcastable_validation_errors() {
    let mut fields = support::qso(support::id::<QsoId>(1)).into_fields();
    fields.end = Some(UtcSecond::from_unix_seconds(999));
    let error = clacks_model::contact::Qso::try_from(fields).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<RecordError>(),
        Some(RecordError::EndBeforeStart { .. })
    ));
    assert_eq!(error.to_string(), "end 999 precedes start 1000");
    assert!(error.root_cause().is::<RecordError>());
}
