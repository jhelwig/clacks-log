mod support;

use clacks_model::export::ExportContext;
use clacks_model::export::rules::ValueExpression;
use clacks_model::export::select::{ContextResolution, MappingSelection, select_mappings};
use clacks_model::group::DefinitionSet;
use clacks_model::ids::{GroupId, OutputProfileId, QsoId};
fn context(target: Option<GroupId>) -> ExportContext {
    ExportContext {
        output_profile_id: support::id::<OutputProfileId>(1),
        target_group_id: target,
    }
}

fn contest_literals(s: &MappingSelection) -> Vec<String> {
    s.groups
        .iter()
        .flat_map(|g| &g.plan.fields)
        .filter(|f| f.field == "CONTEST_ID")
        .filter_map(|f| {
            if let ValueExpression::Literal(v) = &f.value {
                Some(v.clone())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn per_contact_context_is_unambiguous_or_explicit() {
    let defs = support::contest_definitions();
    let profiles = support::profiles();
    for (groups, expected) in [
        (vec![2], Some("CQ-WW-SSB")),
        (vec![3], Some("CONTEST-B")),
        (vec![2, 3], None),
    ] {
        let mut q = support::qso(support::id::<QsoId>(1));
        for g in groups {
            q.own_station
                .references
                .insert(support::reference(support::id::<GroupId>(g), None));
        }
        let before = q.clone();
        let s = select_mappings(&q, &context(None), &defs, &profiles, None).unwrap();
        if let Some(value) = expected {
            assert!(s.decisions.is_empty());
            assert_eq!(contest_literals(&s), vec![value]);
        } else {
            assert_eq!(s.decisions.len(), 1);
            assert!(contest_literals(&s).is_empty());
            let omitted = select_mappings(
                &q,
                &context(None),
                &defs,
                &profiles,
                Some(&ContextResolution::OmitReported),
            )
            .unwrap();
            assert!(omitted.decisions.is_empty());
            assert_eq!(omitted.warnings.len(), 1);
            let chosen = select_mappings(
                &q,
                &context(None),
                &defs,
                &profiles,
                Some(&ContextResolution::ChooseTarget(support::id::<GroupId>(2))),
            )
            .unwrap();
            assert_eq!(contest_literals(&chosen), vec!["CQ-WW-SSB"]);
        }
        assert_eq!(q, before);
    }
    let mut equivalent = defs.get(support::id::<GroupId>(3)).unwrap().clone();
    equivalent.export_mappings.ordinary_interchange[0].value =
        ValueExpression::Literal("CQ-WW-SSB".into());
    let same = DefinitionSet::new(vec![
        defs.get(support::id::<GroupId>(2)).unwrap().clone(),
        equivalent,
    ])
    .unwrap();
    let mut q = support::qso(support::id::<QsoId>(1));
    for id in [2, 3] {
        q.own_station
            .references
            .insert(support::reference(support::id::<GroupId>(id), None));
    }
    assert!(
        select_mappings(&q, &context(None), &same, &profiles, None)
            .unwrap()
            .decisions
            .is_empty()
    );
}

#[test]
fn target_does_not_invent_participation() {
    let mut q = support::qso(support::id::<QsoId>(1));
    q.own_station
        .references
        .insert(support::reference(support::id::<GroupId>(3), None));
    let before = q.clone();
    let selected = select_mappings(
        &q,
        &context(Some(support::id::<GroupId>(2))),
        &support::contest_definitions(),
        &support::profiles(),
        None,
    )
    .unwrap();
    assert!(contest_literals(&selected).is_empty());
    assert_eq!(q, before);
}

#[test]
fn targeted_mapping_excludes_competing_context() {
    let mut q = support::qso(support::id::<QsoId>(1));
    for id in [2, 3] {
        q.own_station
            .references
            .insert(support::reference(support::id::<GroupId>(id), None));
    }
    q.own_station.references.insert(support::reference(
        support::id::<GroupId>(1),
        Some("US-1234"),
    ));
    let selected = select_mappings(
        &q,
        &context(Some(support::id::<GroupId>(2))),
        &support::contest_definitions(),
        &support::profiles(),
        None,
    )
    .unwrap();
    assert_eq!(contest_literals(&selected), vec!["CQ-WW-SSB"]);
    assert!(
        selected
            .groups
            .iter()
            .any(|g| g.plan.fields.iter().any(|f| f.field == "MY_POTA_REF"))
    );
}
