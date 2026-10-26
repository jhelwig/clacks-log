mod support;

use clacks_model::catalog::{CatalogFinding, CatalogSet, lookup_reference};
use clacks_model::group::{DefinitionSet, normalize_reference};
use clacks_model::ids::GroupId;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn explicit_normalization_does_not_rewrite_existing_references() -> TestResult {
    let defs = support::definitions();
    let group = defs.get(support::id::<GroupId>(1)).unwrap();
    let a = support::reference(group.id, Some(" us-1234 "));
    assert_eq!(
        normalize_reference(&a, Some(group))?.identifier.as_deref(),
        Some("US-1234")
    );
    assert_eq!(a.identifier.as_deref(), Some(" us-1234 "));
    assert!(normalize_reference(&support::reference(group.id, None), Some(group)).is_err());
    assert!(
        normalize_reference(
            &support::reference(group.id, Some("junkUS-1234junk")),
            Some(group)
        )
        .is_err()
    );
    assert_eq!(normalize_reference(&a, None)?, a);
    let participation = support::reference(support::id::<GroupId>(2), None);
    assert_eq!(
        normalize_reference(&participation, defs.get(participation.group_id))?,
        participation
    );
    assert_eq!(defs.get(a.group_id).unwrap().id, group.id);
    Ok(())
}

#[test]
fn catalog_lookup_reports_updated_missing_and_unlisted_entries() -> TestResult {
    let support::CatalogCase {
        association,
        definitions: defs,
        mut catalogs,
        id,
    } = support::catalog_case();
    catalogs
        .get_mut(&id)
        .unwrap()
        .entries
        .get_mut("US-1234")
        .unwrap()
        .metadata
        .insert("name".into(), serde_json::json!("Revised park name"));
    match lookup_reference(&association, &defs, &catalogs)? {
        CatalogFinding::Found { entry, .. } => {
            assert_eq!(entry.metadata["name"], "Revised park name")
        }
        other => panic!("{other:?}"),
    }
    let catalog = &catalogs[&id];
    assert!(serde_json::to_value(catalog)?.get("version").is_none());
    assert!(matches!(
        lookup_reference(&association, &defs, &CatalogSet::new())?,
        CatalogFinding::Unavailable { .. }
    ));
    assert!(matches!(
        lookup_reference(
            &support::reference(association.group_id, Some("US-9999")),
            &defs,
            &catalogs
        )?,
        CatalogFinding::NotListed { .. }
    ));
    Ok(())
}

#[test]
fn equivalent_groups_have_equal_capabilities_not_shared_identity() -> TestResult {
    let defs = support::definitions();
    let a = defs.get(support::id::<GroupId>(1)).unwrap();
    let mut b = a.clone();
    b.id = support::id::<GroupId>(3);
    let both = DefinitionSet::new(vec![a.clone(), b.clone()])?;
    for id in [a.id, b.id] {
        assert_eq!(
            normalize_reference(&support::reference(id, Some(" us-1234 ")), both.get(id))?
                .identifier
                .as_deref(),
            Some("US-1234")
        );
    }
    assert_eq!(
        DefinitionSet::new(vec![a.clone(), a.clone()])?.get(a.id),
        Some(a)
    );
    b.id = a.id;
    b.name = "competing definition".into();
    assert!(DefinitionSet::new(vec![a.clone(), b]).is_err());
    let unknown = support::reference(support::id::<GroupId>(100), Some(" unchanged "));
    assert!(defs.get(unknown.group_id).is_none());
    assert_eq!(normalize_reference(&unknown, None)?, unknown);
    Ok(())
}

#[test]
fn catalog_configuration_errors_retain_their_definition_diagnostic() {
    let support::CatalogCase {
        association,
        definitions,
        catalogs,
        ..
    } = support::catalog_case();
    let mut definition = definitions.get(association.group_id).unwrap().clone();
    definition.catalog.as_mut().unwrap().lookup_key =
        clacks_model::export::rules::ValueExpression::Capture {
            value: Box::new(clacks_model::export::rules::ValueExpression::CurrentIdentifier),
            pattern: "[".into(),
            name: "reference".into(),
        };
    let definitions = DefinitionSet::new(vec![definition]).unwrap();
    let error = lookup_reference(&association, &definitions, &catalogs).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<clacks_model::group::DefinitionError>(),
        Some(clacks_model::group::DefinitionError::InvalidDefinition { group_id, path, .. })
            if *group_id == association.group_id && path == "catalog.lookup_key"
    ));
}
