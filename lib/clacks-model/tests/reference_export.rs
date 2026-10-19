mod support;

use clacks_model::contact::Qso;
use clacks_model::export::ExportContext;
use clacks_model::export::evaluate::plan_reference_export;
use clacks_model::export::select::{ContextResolution, ExportError};
use clacks_model::group::DefinitionSet;
use clacks_model::ids::{GroupId, OutputProfileId, QsoId};
use std::collections::BTreeMap;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn contact(group: GroupId) -> Qso {
    let mut q = support::qso(support::id::<QsoId>(1));
    for code in ["US-0001", "US-0002"] {
        q.own_station
            .references
            .insert(support::reference(group, Some(code)));
    }
    for code in ["US-0001", "US-0002", "US-0003"] {
        q.contacted_station
            .references
            .insert(support::reference(group, Some(code)));
    }
    q
}

fn context(targeted: bool, group: GroupId) -> ExportContext {
    ExportContext {
        output_profile_id: support::id::<OutputProfileId>(if targeted { 2 } else { 1 }),
        target_group_id: targeted.then_some(group),
    }
}

#[test]
fn ordinary_export_does_not_expand() -> TestResult {
    let q = contact(support::id::<GroupId>(1));
    let plan = plan_reference_export(
        &[q.clone(), q],
        &context(false, support::id::<GroupId>(1)),
        &support::definitions(),
        &support::profiles(),
        &BTreeMap::new(),
    )?;
    assert_eq!(plan.source_contact_count, 1);
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].entries.len(), 1);
    let fields = &plan.files[0].entries[0].reference_fields;
    assert_eq!(fields["MY_POTA_REF"], "US-0001,US-0002");
    assert_eq!(fields["POTA_REF"], "US-0001,US-0002,US-0003");
    Ok(())
}

#[test]
fn two_by_three_expands_only_target_group() -> TestResult {
    let mut q = contact(support::id::<GroupId>(1));
    for id in [2, 3] {
        q.own_station
            .references
            .insert(support::reference(support::id::<GroupId>(id), None));
    }
    let before = serde_json::to_value(&q)?;
    let plan = plan_reference_export(
        std::slice::from_ref(&q),
        &context(true, support::id::<GroupId>(1)),
        &support::contest_definitions(),
        &support::profiles(),
        &BTreeMap::new(),
    )?;
    assert_eq!(plan.source_contact_count, 1);
    assert_eq!(plan.files.len(), 2);
    for (file, own) in plan.files.iter().zip(["US-0001", "US-0002"]) {
        assert_eq!(file.entries.len(), 3);
        for (entry, other) in file.entries.iter().zip(["US-0001", "US-0002", "US-0003"]) {
            assert_eq!(entry.qso_id, q.id);
            assert_eq!(entry.reference_fields["MY_SIG_INFO"], own);
            assert_eq!(entry.reference_fields["SIG_INFO"], other);
            assert!(!entry.reference_fields.contains_key("CONTEST_ID"));
        }
    }
    assert_eq!(serde_json::to_value(&q)?, before);
    Ok(())
}

#[test]
fn hunter_contact_retains_one_entry() -> TestResult {
    let mut q = contact(support::id::<GroupId>(1));
    q.contacted_station.references.clear();
    let plan = plan_reference_export(
        &[q],
        &context(true, support::id::<GroupId>(1)),
        &support::definitions(),
        &support::profiles(),
        &BTreeMap::new(),
    )?;
    assert_eq!(plan.files.len(), 2);
    for file in plan.files {
        assert_eq!(file.entries.len(), 1);
        assert!(!file.entries[0].reference_fields.contains_key("SIG"));
        assert!(!file.entries[0].reference_fields.contains_key("SIG_INFO"));
    }
    Ok(())
}

#[test]
fn omission_is_explicit_and_reported() -> TestResult {
    let mut q = support::qso(support::id::<QsoId>(1));
    for id in [2, 3] {
        q.own_station
            .references
            .insert(support::reference(support::id::<GroupId>(id), None));
    }
    let plan = plan_reference_export(
        std::slice::from_ref(&q),
        &context(false, support::id::<GroupId>(1)),
        &support::contest_definitions(),
        &support::profiles(),
        &[(q.id, ContextResolution::OmitReported)].into(),
    )?;
    assert!(plan.decisions.is_empty());
    assert_eq!(plan.warnings.len(), 1);
    assert!(
        !plan.files[0].entries[0]
            .reference_fields
            .contains_key("CONTEST_ID")
    );
    let unresolved = plan_reference_export(
        &[q],
        &context(false, support::id::<GroupId>(1)),
        &support::contest_definitions(),
        &support::profiles(),
        &BTreeMap::new(),
    )?;
    assert_eq!(unresolved.decisions.len(), 1);
    assert!(
        !unresolved.files[0].entries[0]
            .reference_fields
            .contains_key("CONTEST_ID")
    );
    Ok(())
}

#[test]
fn no_program_name_switch() -> TestResult {
    let defs = support::definitions();
    let mut generic = defs.get(support::id::<GroupId>(1)).unwrap().clone();
    generic.id = support::id::<GroupId>(4);
    generic.name = "User group with equivalent capabilities".into();
    let plan = plan_reference_export(
        &[contact(generic.id)],
        &context(true, generic.id),
        &DefinitionSet::new(vec![generic])?,
        &support::profiles(),
        &BTreeMap::new(),
    )?;
    assert_eq!(plan.files.len(), 2);
    assert!(plan.files.iter().all(|f| f.entries.len() == 3));
    Ok(())
}

#[test]
fn missing_partition_is_not_silent_filtering() {
    let mut q = contact(support::id::<GroupId>(1));
    q.own_station.references.clear();
    let error = plan_reference_export(
        &[q],
        &context(true, support::id::<GroupId>(1)),
        &support::definitions(),
        &support::profiles(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ExportError>(),
        Some(ExportError::NotReady { .. })
    ));
    let q = contact(support::id::<GroupId>(1));
    let mut competing = q.clone();
    competing.comment = Some("different snapshot".into());
    assert!(
        plan_reference_export(
            &[q, competing],
            &context(false, support::id::<GroupId>(1)),
            &support::definitions(),
            &support::profiles(),
            &BTreeMap::new()
        )
        .is_err()
    );
}

#[test]
fn shared_definitions_keep_each_contacts_reference_values() -> TestResult {
    let group = support::id::<GroupId>(1);
    let mut first = support::qso(support::id::<QsoId>(1));
    first
        .own_station
        .references
        .insert(support::reference(group, Some("US-1001")));
    first
        .contacted_station
        .references
        .insert(support::reference(group, Some("US-2001")));
    let mut second = support::qso(support::id::<QsoId>(2));
    second
        .own_station
        .references
        .insert(support::reference(group, Some("US-1002")));
    second
        .contacted_station
        .references
        .insert(support::reference(group, Some("US-2002")));
    for (targeted, own_field, other_field) in [
        (false, "MY_POTA_REF", "POTA_REF"),
        (true, "MY_SIG_INFO", "SIG_INFO"),
    ] {
        let plan = plan_reference_export(
            &[first.clone(), second.clone()],
            &context(targeted, group),
            &support::definitions(),
            &support::profiles(),
            &BTreeMap::new(),
        )?;
        assert_eq!(plan.source_contact_count, 2);
        let rows: BTreeMap<_, _> = plan
            .files
            .iter()
            .flat_map(|file| &file.entries)
            .map(|entry| (entry.qso_id, &entry.reference_fields))
            .collect();
        assert_eq!(rows.len(), 2);
        for (id, own, other) in [
            (first.id, "US-1001", "US-2001"),
            (second.id, "US-1002", "US-2002"),
        ] {
            let fields = rows.get(&id).unwrap();
            assert_eq!(fields.get(own_field).map(String::as_str), Some(own));
            assert_eq!(fields.get(other_field).map(String::as_str), Some(other));
        }
    }
    Ok(())
}
