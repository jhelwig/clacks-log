use super::*;
use crate::test_support as support;
use crate::{
    payload,
    store::{ClientStore, StoreSettings},
};
use clacks_model::export::rules::{
    BindingCondition, ExpansionRule, FieldBinding, GroupExportPlan, PartitionRule, QsoFieldPath,
    ValueExpression,
};
use clacks_model::{
    group::{IdentifierDefinition, TextNormalization},
    reference::StationSide,
};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};

fn id<T: std::str::FromStr>(n: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(n).to_string().parse().unwrap()
}

#[tokio::test]
async fn restored_definitions_plan_reference_output() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let capture = |value| ValueExpression::Capture {
        value: Box::new(value),
        pattern: "(?P<key>.+)".into(),
        name: "key".into(),
    };
    let group = GroupDefinition::try_from(clacks_model::group::GroupDefinitionFields {
        id: id(80),
        name: "Field event".into(),
        identifier: IdentifierDefinition {
            required: false,
            normalization: vec![TextNormalization::Trim, TextNormalization::Uppercase],
            syntax: None,
        },
        bears_location_context: true,
        catalog: Some(clacks_model::catalog::CatalogBinding {
            dataset_id: id(81),
            lookup_key: capture(ValueExpression::CurrentIdentifier),
            display_name: "name".into(),
            details: vec![clacks_model::catalog::MetadataDisplayBinding {
                label: "Region".into(),
                field: "region".into(),
            }],
        }),
        export_mappings: GroupExportMappings {
            ordinary_interchange: vec![],
            targeted: [(
                id(82),
                GroupExportPlan {
                    fields: vec![
                        FieldBinding {
                            field: "MY_REF".into(),
                            value: ValueExpression::Join {
                                values: Box::new(ValueExpression::Identifiers(StationSide::Own)),
                                separator: ",".into(),
                            },
                            when: BindingCondition::BoundPresent("own_ref".into()),
                        },
                        FieldBinding {
                            field: "REF".into(),
                            value: capture(ValueExpression::BoundIdentifier("dx_ref".into())),
                            when: BindingCondition::BoundPresent("dx_ref".into()),
                        },
                    ],
                    file_partition: vec![
                        PartitionRule::EachReference {
                            side: StationSide::Own,
                            binding: "own_ref".into(),
                        },
                        PartitionRule::ByValue(ValueExpression::QsoField(
                            QsoFieldPath::OwnCallsign,
                        )),
                    ],
                    entry_expansion: vec![ExpansionRule {
                        side: StationSide::Contacted,
                        binding: "dx_ref".into(),
                        keep_one_if_empty: true,
                    }],
                },
            )]
            .into(),
        },
    })
    .expect("valid group fixture");
    let key = RecordKey {
        account: id(1),
        id: RecordId::Group(group.id()),
    };
    let state = RecordState::Live(RecordValue::Group(Box::new(group)));
    let mut tx = store.write().await?;
    let root = payload::insert(&mut tx, key, &state).await?;
    let profile = clacks_model::export::OutputProfile {
        id: id(82),
        name: "Targeted event".into(),
        purpose: clacks_model::export::OutputPurpose::Targeted,
        contextual_fields: ["MY_REF".into(), "REF".into()].into(),
    };
    let profile_key = RecordKey {
        account: id(1),
        id: RecordId::OutputProfile(profile.id),
    };
    let profile_root = payload::insert(
        &mut tx,
        profile_key,
        &RecordState::Live(RecordValue::OutputProfile(Box::new(profile))),
    )
    .await?;
    tx.commit().await?;
    store.close().await?;
    let store = ClientStore::open(
        &dir.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let mut tx = store.write().await?;
    let RecordState::Live(RecordValue::Group(group)) = payload::load(&mut tx, key, root).await?
    else {
        panic!("restored definition")
    };
    let RecordState::Live(RecordValue::OutputProfile(profile)) =
        payload::load(&mut tx, profile_key, profile_root).await?
    else {
        panic!("restored output profile")
    };
    let association =
        clacks_model::reference::ReferenceAssociation::new(id(80), Some(" a ".into()))?;
    let normalized = clacks_model::group::normalize_reference(&association, Some(&group))?;
    assert_eq!(normalized.identifier().as_deref(), Some("A"));
    assert_eq!(association.identifier().as_deref(), Some(" a "));
    let unknown =
        clacks_model::reference::ReferenceAssociation::new(id(999), Some(" old ".into()))?;
    assert_eq!(
        clacks_model::group::normalize_reference(&unknown, None)?,
        unknown
    );
    let compiled = clacks_model::export::rules::compile_definition(&group)?;
    let binding = group.catalog().as_ref().unwrap();
    assert_eq!(binding.display_name, "name");
    assert_eq!(binding.details.first().unwrap().label, "Region");
    assert_eq!(
        compiled.evaluate(
            &binding.lookup_key,
            &clacks_model::export::rules::ExpressionContext {
                qso: None,
                current_identifier: Some("A"),
                bindings: &Default::default(),
            }
        )?,
        clacks_model::export::rules::ExpressionValue::Scalar("A".into())
    );

    let mut qso = support::qso(id(90));
    for identifier in ["A", "B"] {
        support::edit(&mut qso, |qso| {
            qso.own_station.references.insert(
                clacks_model::reference::ReferenceAssociation::new(id(80), Some(identifier.into()))
                    .expect("valid reference fixture"),
            );
            Ok(())
        });
    }
    for identifier in ["X", "Y"] {
        support::edit(&mut qso, |qso| {
            qso.contacted_station.references.insert(
                clacks_model::reference::ReferenceAssociation::new(id(80), Some(identifier.into()))
                    .expect("valid reference fixture"),
            );
            Ok(())
        });
    }
    let output = clacks_model::export::evaluate::plan_reference_export(
        &[qso],
        &clacks_model::export::ExportContext {
            output_profile_id: id(82),
            target_group_id: Some(id(80)),
        },
        &clacks_model::group::DefinitionSet::new(vec![*group])?,
        &[(profile.id, *profile)].into(),
        &Default::default(),
    )?;
    assert!(output.decisions.is_empty());
    assert!(output.warnings.is_empty());
    let expected: Vec<_> = ["A", "B"]
        .into_iter()
        .map(|own| clacks_model::export::evaluate::ReferenceExportFile {
            partition_key: vec![
                id::<clacks_model::ids::OutputProfileId>(82).to_string(),
                id::<clacks_model::ids::GroupId>(80).to_string(),
                own.into(),
                "W7HEX/P".into(),
            ],
            entries: ["X", "Y"]
                .into_iter()
                .map(|dx| clacks_model::export::evaluate::ReferenceExportEntry {
                    qso_id: id(90),
                    reference_fields: [("MY_REF".into(), "A,B".into()), ("REF".into(), dx.into())]
                        .into(),
                })
                .collect(),
        })
        .collect();
    assert_eq!(output.files, expected);
    tx.commit().await?;
    store.close().await?;
    Ok(())
}
