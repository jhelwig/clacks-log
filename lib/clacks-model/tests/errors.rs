mod support;

use clacks_model::contact::RecordError;
use clacks_model::export::ExportContext;
use clacks_model::export::evaluate::plan_reference_export;
use clacks_model::export::select::{ExportError, select_mappings};
use clacks_model::group::{DefinitionError, DefinitionSet};
use clacks_model::ids::{OutputProfileId, QsoId};
use clacks_model::scalar::UtcSecond;

#[test]
fn export_operations_propagate_definition_errors_without_forwarding_variants() {
    let mut group = support::definitions().iter().next().unwrap().clone();
    group.identifier.syntax = Some("[".into());
    let mut qso = support::qso(support::id::<QsoId>(1));
    qso.own_station
        .references
        .insert(support::reference(group.id, Some("test")));
    let definitions = DefinitionSet::new(vec![group]).unwrap();
    let error = select_mappings(
        &qso,
        &ExportContext {
            output_profile_id: support::id::<OutputProfileId>(1),
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
    let mut qso = support::qso(support::id::<QsoId>(1));
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
    let mut qso = support::qso(support::id::<QsoId>(1));
    qso.end = Some(UtcSecond::from_unix_seconds(999));
    let error = plan_reference_export(
        &[qso],
        &ExportContext {
            output_profile_id: support::id::<OutputProfileId>(1),
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
