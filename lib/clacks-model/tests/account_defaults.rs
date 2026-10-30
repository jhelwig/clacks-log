use crate::support;

use clacks_model::defaults::account_definitions;
use clacks_model::export::evaluate::plan_reference_export;
use clacks_model::export::rules::compile_definition;
use clacks_model::export::{ExportContext, OutputPurpose};
use clacks_model::group::DefinitionSet;
use clacks_model::ids::QsoId;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn bundled_definitions_compile() -> anyhow::Result<()> {
    let defaults = account_definitions()?;

    assert!(
        defaults.groups.len() > 100,
        "complete contest baseline required"
    );
    for group in &defaults.groups {
        compile_definition(group)?;
    }
    Ok(())
}

#[test]
fn bundled_ids_are_unique_and_mapping_targets_exist() -> anyhow::Result<()> {
    let first = account_definitions()?;

    assert_eq!(
        first
            .groups
            .iter()
            .map(|group| group.id())
            .collect::<BTreeSet<_>>()
            .len(),
        first.groups.len()
    );
    let profiles: BTreeSet<_> = first
        .output_profiles
        .iter()
        .map(|profile| profile.id)
        .collect();

    assert_eq!(profiles.len(), first.output_profiles.len());
    for group in &first.groups {
        for target in group.export_mappings().targeted.keys() {
            assert!(profiles.contains(target));
        }
    }
    Ok(())
}

#[test]
fn default_reference_rules_preserve_native_values() -> anyhow::Result<()> {
    let defaults = account_definitions()?;
    let pota = defaults
        .groups
        .iter()
        .find(|group| group.name() == "POTA")
        .unwrap();

    assert!(pota.bears_location_context());
    let compiled = compile_definition(pota)?;
    let entered = clacks_model::reference::ReferenceAssociation::new(
        pota.id(),
        Some(" us-0001@us-ca ".into()),
    )
    .expect("valid reference fixture");
    let original = entered.clone();
    let prepared = compiled.normalize_reference(&entered)?;

    assert_eq!(prepared.identifier().as_deref(), Some("US-0001@US-CA"));
    assert_eq!(entered, original);
    assert!(
        compiled
            .normalize_reference(&support::reference(pota.id(), Some("not a reference")))
            .is_err()
    );
    Ok(())
}

#[test]
fn pota_mapping_uses_separate_export_contexts() -> anyhow::Result<()> {
    let defaults = account_definitions()?;
    let pota = defaults
        .groups
        .iter()
        .find(|group| group.name() == "POTA")
        .unwrap();
    let mut qso = support::qso(support::id::<QsoId>(77));
    for code in ["US-0001", "US-0002"] {
        support::edit(&mut qso, |qso| {
            qso.own_station
                .references
                .insert(support::reference(pota.id(), Some(code)));

            Ok(())
        });
    }
    for code in ["US-0003", "US-0004", "US-0005"] {
        support::edit(&mut qso, |qso| {
            qso.contacted_station
                .references
                .insert(support::reference(pota.id(), Some(code)));

            Ok(())
        });
    }
    let profiles = defaults
        .output_profiles
        .iter()
        .map(|profile| (profile.id, profile.clone()))
        .collect();
    let definitions = DefinitionSet::new(defaults.groups.clone())?;
    for profile in &defaults.output_profiles {
        let targeted = profile.purpose == OutputPurpose::Targeted;
        let context = ExportContext {
            output_profile_id: profile.id,
            target_group_id: targeted.then_some(pota.id()),
        };
        let plan = plan_reference_export(
            std::slice::from_ref(&qso),
            &context,
            &definitions,
            &profiles,
            &BTreeMap::new(),
        )?;

        assert_eq!(plan.files.len(), if targeted { 2 } else { 1 });
        for file in &plan.files {
            assert_eq!(file.entries.len(), if targeted { 3 } else { 1 });
            for entry in &file.entries {
                assert!(entry.reference_fields.contains_key(if targeted {
                    "MY_SIG_INFO"
                } else {
                    "MY_POTA_REF"
                }));
            }
        }
    }
    Ok(())
}

#[test]
fn competing_contests_need_an_explicit_context() -> anyhow::Result<()> {
    let defaults = account_definitions()?;
    let mut qso = support::qso(support::id::<QsoId>(88));
    let mut contests = defaults
        .groups
        .iter()
        .filter(|group| !group.bears_location_context());
    let first = contests.next().unwrap();
    let second = contests.next().unwrap();
    support::edit(&mut qso, |qso| {
        qso.own_station
            .references
            .insert(support::reference(first.id(), None));

        Ok(())
    });
    support::edit(&mut qso, |qso| {
        qso.own_station
            .references
            .insert(support::reference(second.id(), None));

        Ok(())
    });
    let profile = defaults
        .output_profiles
        .iter()
        .find(|profile| profile.purpose == OutputPurpose::OrdinaryInterchange)
        .unwrap();
    let profiles = defaults
        .output_profiles
        .iter()
        .map(|profile| (profile.id, profile.clone()))
        .collect();
    let definitions = DefinitionSet::new(defaults.groups.clone())?;
    let context = ExportContext {
        output_profile_id: profile.id,
        target_group_id: None,
    };
    let selection = clacks_model::export::select::select_mappings(
        &qso,
        &context,
        &definitions,
        &profiles,
        None,
    )?;

    assert!(!selection.decisions.is_empty());
    let context = ExportContext {
        output_profile_id: profile.id,
        target_group_id: Some(first.id()),
    };
    let selection = clacks_model::export::select::select_mappings(
        &qso,
        &context,
        &definitions,
        &profiles,
        None,
    )?;

    assert!(selection.decisions.is_empty());
    Ok(())
}

#[test]
fn unknown_activation_subdivision_has_its_own_partition() -> anyhow::Result<()> {
    let defaults = account_definitions()?;
    let pota = defaults
        .groups
        .iter()
        .find(|group| group.name() == "POTA")
        .unwrap();
    let profile = defaults
        .output_profiles
        .iter()
        .find(|profile| profile.purpose == OutputPurpose::Targeted)
        .unwrap();
    let mut unknown = support::qso(support::id::<QsoId>(90));
    support::edit(&mut unknown, |unknown| {
        unknown
            .own_station
            .references
            .insert(support::reference(pota.id(), Some("US-0001")));

        Ok(())
    });
    let mut known = unknown.clone();
    support::edit(&mut known, |known| {
        known.id = support::id::<QsoId>(91);

        Ok(())
    });
    support::edit(&mut known, |known| {
        known.own_station.location = Some(
            (clacks_model::location::RecordedLocationFields {
                primary_subdivision: Some("CA".into()),
                ..Default::default()
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });
    let profiles = defaults
        .output_profiles
        .iter()
        .map(|profile| (profile.id, profile.clone()))
        .collect();
    let definitions = DefinitionSet::new(defaults.groups.clone())?;
    let context = ExportContext {
        output_profile_id: profile.id,
        target_group_id: Some(pota.id()),
    };
    let plan = plan_reference_export(
        &[unknown, known],
        &context,
        &definitions,
        &profiles,
        &BTreeMap::new(),
    )?;

    assert_eq!(plan.files.len(), 2);
    let mut states = BTreeSet::new();
    for file in &plan.files {
        assert_eq!(file.entries.len(), 1);
        states.insert(
            file.entries
                .first()
                .unwrap()
                .reference_fields
                .get("MY_STATE")
                .cloned(),
        );
    }

    assert_eq!(states, [None, Some("CA".into())].into());
    Ok(())
}

#[test]
fn invalid_initial_definitions_are_rejected_before_installation() -> anyhow::Result<()> {
    let mut defaults = account_definitions()?;
    defaults
        .groups
        .push(defaults.groups.first().unwrap().clone());

    assert!(defaults.validate().is_err());
    let mut defaults = account_definitions()?;
    defaults.output_profiles.clear();

    assert!(defaults.validate().is_err());
    Ok(())
}
