mod support;

use clacks_model::contact::{OperatorInformation, Qso};
use clacks_model::export::ExportContext;
use clacks_model::export::evaluate::plan_reference_export;
use clacks_model::export::select::select_mappings;
use clacks_model::group::DefinitionSet;
use clacks_model::ids::{GroupId, OutputProfileId, QsoId};
use clacks_model::location::RecordedLocation;
use std::collections::BTreeMap;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn empty_aggregates_require_normalization_before_saving() -> TestResult {
    let base = support::qso(QsoId::from_u128(1));
    for field in 0..4 {
        let mut q = base.clone();
        match field {
            0 => q.own_station.location = Some(RecordedLocation::default()),
            1 => q.contacted_station.location = Some(RecordedLocation::default()),
            2 => q.own_context.operator = Some(OperatorInformation::default()),
            _ => q.contacted_operator = Some(OperatorInformation::default()),
        }
        assert!(
            q.validate().is_err(),
            "noncanonical aggregate {field} reached the save boundary"
        );
        q.normalize_optional_aggregates();
        q.validate()?;
        assert!(q.same_recorded_values(&base));
        assert_eq!(q, base);
    }
    Ok(())
}

#[test]
fn empty_receive_override_normalizes_to_primary_fallback() -> TestResult {
    let base = support::qso(QsoId::from_u128(1));
    let mut json = serde_json::to_value(&base)?;
    json["receive_override"] = serde_json::json!({"frequency":null,"band":null});
    let mut received: Qso = serde_json::from_value(json)?;
    received.normalize_optional_aggregates();
    assert!(received.receive_override.is_none());
    received.validate()?;
    assert_eq!(
        received
            .primary
            .effective_receive(received.receive_override.as_ref()),
        &base.primary
    );
    assert_eq!(received, base);
    Ok(())
}

#[test]
fn unrelated_invalid_group_does_not_block_targeted_export() -> TestResult {
    let defs = support::definitions();
    let definitions = DefinitionSet::new(
        defs.iter()
            .cloned()
            .map(|mut d| {
                if d.id == GroupId::from_u128(2) {
                    d.identifier.syntax = Some("[".into());
                }
                d
            })
            .collect(),
    )?;
    let mut q = support::qso(QsoId::from_u128(1));
    q.own_station
        .references
        .insert(support::reference(GroupId::from_u128(1), Some("US-0001")));
    q.own_station
        .references
        .insert(support::reference(GroupId::from_u128(2), None));
    let context = ExportContext {
        output_profile_id: OutputProfileId::from_u128(2),
        target_group_id: Some(GroupId::from_u128(1)),
    };
    let result = plan_reference_export(
        std::slice::from_ref(&q),
        &context,
        &definitions,
        &support::profiles(),
        &BTreeMap::new(),
    );
    assert!(result.is_ok(), "unrelated group blocked target: {result:?}");
    let plan = result?;
    assert_eq!(plan.source_contact_count, 1);
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].entries.len(), 1);
    assert!(
        select_mappings(
            &q,
            &ExportContext {
                output_profile_id: OutputProfileId::from_u128(1),
                target_group_id: None
            },
            &definitions,
            &support::profiles(),
            None
        )
        .is_err()
    );
    Ok(())
}
