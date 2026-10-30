use crate::support;

use clacks_model::contact::RecordError;
use clacks_model::export::ExportContext;

use clacks_model::export::select::select_mappings;
use clacks_model::group::{DefinitionError, DefinitionSet};
use clacks_model::ids::{OutputProfileId, QsoId};
use clacks_model::scalar::UtcSecond;

#[test]
fn export_operations_propagate_definition_errors_without_forwarding_variants() {
    let mut group = support::definitions().iter().next().unwrap().clone();
    group.identifier.syntax = Some("[".into());
    let mut qso = support::qso(support::id::<QsoId>(1));
    support::edit(&mut qso, |qso| {
        qso.own_station
            .references
            .insert(support::reference(group.id, Some("test")));

        Ok(())
    });
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
