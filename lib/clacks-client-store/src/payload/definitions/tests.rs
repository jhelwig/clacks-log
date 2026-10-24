use super::*;
use crate::{
    payload,
    store::{ClientStore, StoreSettings},
};
use clacks_model::export::rules::{
    BindingCondition, ExpansionRule, FieldBinding, GroupExportPlan, PartitionRule, QsoFieldPath,
    ValueExpression, compile_definition,
};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};

fn id<T: std::str::FromStr>(n: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(n).to_string().parse().unwrap()
}

#[tokio::test]
async fn definitions_round_trip_and_cycles_are_rejected() -> anyhow::Result<()> {
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
    let group = GroupDefinition {
        id: id(80),
        name: "Field event".into(),
        identifier: IdentifierDefinition {
            required: false,
            normalization: vec![
                TextNormalization::Trim,
                TextNormalization::Lowercase,
                TextNormalization::Uppercase,
            ],
            syntax: Some("[A-Z0-9-]+".into()),
        },
        bears_location_context: true,
        catalog: Some(CatalogBinding {
            dataset_id: id(81),
            lookup_key: capture(ValueExpression::CurrentIdentifier),
            display_name: "name".into(),
            details: vec![
                MetadataDisplayBinding {
                    label: "Region".into(),
                    field: "region".into(),
                },
                MetadataDisplayBinding {
                    label: "Status".into(),
                    field: "status".into(),
                },
            ],
        }),
        export_mappings: GroupExportMappings {
            ordinary_interchange: vec![
                FieldBinding {
                    field: "MY_REFS".into(),
                    value: ValueExpression::Join {
                        values: Box::new(capture(ValueExpression::Identifiers(StationSide::Own))),
                        separator: ",".into(),
                    },
                    when: BindingCondition::AssociationPresent(StationSide::Own),
                },
                FieldBinding {
                    field: "EVENT".into(),
                    value: ValueExpression::Literal("Field event".into()),
                    when: BindingCondition::Always,
                },
                FieldBinding {
                    field: "MY_CALL".into(),
                    value: ValueExpression::QsoField(QsoFieldPath::OwnCallsign),
                    when: BindingCondition::Always,
                },
                FieldBinding {
                    field: "CALL".into(),
                    value: ValueExpression::QsoField(QsoFieldPath::ContactedCallsign),
                    when: BindingCondition::Always,
                },
                FieldBinding {
                    field: "MY_STATE".into(),
                    value: ValueExpression::QsoField(QsoFieldPath::OwnPrimarySubdivision),
                    when: BindingCondition::Always,
                },
                FieldBinding {
                    field: "STATE".into(),
                    value: ValueExpression::QsoField(QsoFieldPath::ContactedPrimarySubdivision),
                    when: BindingCondition::Always,
                },
            ],
            targeted: [(
                id(82),
                GroupExportPlan {
                    fields: vec![
                        FieldBinding {
                            field: "MY_REF".into(),
                            value: ValueExpression::BoundIdentifier("own_ref".into()),
                            when: BindingCondition::BoundPresent("own_ref".into()),
                        },
                        FieldBinding {
                            field: "REF".into(),
                            value: ValueExpression::BoundIdentifier("dx_ref".into()),
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
    };
    compile_definition(&group)?;
    let key = RecordKey {
        account: id(1),
        id: RecordId::Group(group.id),
    };
    let state = RecordState::Live(RecordValue::Group(Box::new(group)));
    let mut tx = store.write().await?;
    let root = payload::insert(&mut tx, key, &state).await?;
    assert_eq!(payload::load(&mut tx, key, root).await?, state);
    tx.commit().await?;
    store.close().await?;
    let store = ClientStore::open(
        &dir.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let mut tx = store.write().await?;
    assert_eq!(payload::load(&mut tx, key, root).await?, state);
    sqlx::query_file!("tests/sql/cycle_expression.sql", root)
        .execute(&mut *tx)
        .await?;
    let error = payload::load(&mut tx, key, root).await.unwrap_err();
    let diagnostic = format!("{error:#}");
    assert!(diagnostic.contains(&key.id.to_string()), "{diagnostic}");
    assert!(diagnostic.contains("cycle"), "{diagnostic}");
    tx.rollback().await?;
    let mut connection = store.pool.acquire().await?;
    assert_eq!(payload::load(&mut connection, key, root).await?, state);
    drop(connection);
    let mut tx = store.write().await?;
    sqlx::query_file!("tests/sql/orphan_expression.sql", root)
        .execute(&mut *tx)
        .await?;
    assert!(payload::load(&mut tx, key, root).await.is_err());
    tx.rollback().await?;
    store.close().await?;
    Ok(())
}
