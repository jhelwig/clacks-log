mod support;

use clacks_model::catalog::{CatalogFinding, CatalogSet, ReferenceCatalog, lookup_reference};
use clacks_model::group::{DefinitionSet, interpret_reference, normalize_reference};
use clacks_model::ids::GroupId;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn explicit_normalization_does_not_rewrite_existing_references() -> TestResult {
    let defs = support::definitions();
    let group = defs.get(GroupId::from_u128(1)).unwrap();
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
    let participation = support::reference(GroupId::from_u128(2), None);
    assert_eq!(
        normalize_reference(&participation, defs.get(participation.group_id))?,
        participation
    );
    assert_eq!(interpret_reference(&a, &defs).unwrap().id, group.id);
    Ok(())
}

#[test]
fn a_catalog_refresh_is_not_a_qso_edit() -> TestResult {
    let (contact, association, defs, mut catalogs, id) = support::catalog_case();
    let before = serde_json::to_value(&contact)?;
    catalogs
        .get_mut(&id)
        .unwrap()
        .entries
        .get_mut("US-1234")
        .unwrap()
        .metadata
        .insert("name".into(), serde_json::json!("Revised park name"));
    match lookup_reference(&association, &defs, &catalogs) {
        CatalogFinding::Found { entry, .. } => {
            assert_eq!(entry.metadata["name"], "Revised park name")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(serde_json::to_value(&contact)?, before);
    assert!(contact.contacted_station.location.is_none());
    let catalog = &catalogs[&id];
    assert_eq!(
        serde_json::from_str::<ReferenceCatalog>(&serde_json::to_string(catalog)?)?,
        *catalog
    );
    assert!(serde_json::to_value(catalog)?.get("version").is_none());
    assert!(matches!(
        lookup_reference(&association, &defs, &CatalogSet::new()),
        CatalogFinding::Unavailable { .. }
    ));
    assert!(matches!(
        lookup_reference(
            &support::reference(association.group_id, Some("US-9999")),
            &defs,
            &catalogs
        ),
        CatalogFinding::NotListed { .. }
    ));
    Ok(())
}

#[test]
fn equivalent_groups_have_equal_capabilities_not_shared_identity() -> TestResult {
    let defs = support::definitions();
    let a = defs.get(GroupId::from_u128(1)).unwrap();
    let mut b = a.clone();
    b.id = GroupId::from_u128(3);
    let both = DefinitionSet::new(vec![a.clone(), b.clone()])?;
    for id in [a.id, b.id] {
        assert_eq!(
            normalize_reference(&support::reference(id, Some(" us-1234 ")), both.get(id))?
                .identifier
                .as_deref(),
            Some("US-1234")
        );
    }
    b.id = a.id;
    b.name = "competing definition".into();
    assert!(DefinitionSet::new(vec![a.clone(), b]).is_err());
    let unknown = support::reference(GroupId::from_u128(100), Some(" unchanged "));
    assert!(interpret_reference(&unknown, &defs).is_none());
    assert_eq!(normalize_reference(&unknown, None)?, unknown);
    Ok(())
}
