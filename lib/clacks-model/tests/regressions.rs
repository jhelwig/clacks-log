use crate::support;

use clacks_model::contact::Qso;
use clacks_model::export::ExportContext;
use clacks_model::export::evaluate::plan_reference_export;
use clacks_model::export::select::select_mappings;
use clacks_model::group::DefinitionSet;
use clacks_model::ids::{GroupId, OutputProfileId, QsoId};
use std::collections::BTreeMap;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn empty_present_aggregates_are_rejected_as_saved_contacts() -> TestResult {
    let base = support::qso(support::id::<QsoId>(1));
    for path in [
        "/own_station/location",
        "/contacted_station/location",
        "/own_context/operator",
        "/contacted_operator",
    ] {
        let mut input = serde_json::to_value(&base)?;
        *input.pointer_mut(path).unwrap() = serde_json::json!({});
        assert!(
            serde_json::from_value::<Qso>(input).is_err(),
            "accepted empty {path}"
        );
    }
    Ok(())
}

#[test]
fn empty_receive_override_is_rejected_at_input_boundary() -> TestResult {
    let base = support::qso(support::id::<QsoId>(1));
    let mut json = serde_json::to_value(&base)?;
    json["receive_override"] = serde_json::json!({"frequency":null,"band":null});
    assert!(serde_json::from_value::<Qso>(json).is_err());
    Ok(())
}

#[test]
fn unrelated_invalid_group_does_not_block_targeted_export() -> TestResult {
    let defs = support::definitions();
    let definitions = DefinitionSet::new(
        defs.iter()
            .cloned()
            .map(|mut d| {
                if d.id == support::id::<GroupId>(2) {
                    d.identifier.syntax = Some("[".into());
                }
                d
            })
            .collect(),
    )?;
    let mut q = support::qso(support::id::<QsoId>(1));
    support::edit(&mut q, |q| {
        q.own_station.references.insert(support::reference(
            support::id::<GroupId>(1),
            Some("US-0001"),
        ));

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.own_station
            .references
            .insert(support::reference(support::id::<GroupId>(2), None));

        Ok(())
    });
    let context = ExportContext {
        output_profile_id: support::id::<OutputProfileId>(2),
        target_group_id: Some(support::id::<GroupId>(1)),
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
                output_profile_id: support::id::<OutputProfileId>(1),
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
