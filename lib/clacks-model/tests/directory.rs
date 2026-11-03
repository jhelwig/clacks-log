use crate::support;
use clacks_model::{
    directory::{
        record::DirectoryRecord,
        values::{
            DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields,
            LookupKey,
        },
    },
    ids::QsoId,
    scalar::{RecordedCallsign, UtcTimestamp},
};

#[test]
fn directory_key_and_restoration() -> anyhow::Result<()> {
    assert_eq!(
        LookupKey::from_recorded(&RecordedCallsign::new(" OK2CQR/p ")?).as_str(),
        "OK2CQR"
    );
    assert_eq!(
        LookupKey::from_recorded(&RecordedCallsign::new("EA8/OK2CQR")?).as_str(),
        "EA8/OK2CQR"
    );
    assert_eq!(
        serde_json::from_str::<LookupKey>("\"EA8/OK2CQR/P\"")?.as_str(),
        "EA8/OK2CQR/P"
    );
    Ok(())
}

fn snapshot(
    key: &str,
    second: i64,
    id: u128,
    name: Option<&str>,
) -> anyhow::Result<DirectorySnapshot> {
    DirectorySnapshot::new(DirectorySnapshotFields {
        provider: DirectoryProvider::Qrz,
        key: LookupKey::from_recorded(&RecordedCallsign::new(key)?),
        lookup_time: UtcTimestamp::new(second, 0)?,
        lookup_id: support::id(id),
        facts: DirectoryFacts {
            name: name.map(str::to_owned),
            ..Default::default()
        },
    })
}

#[test]
fn directory_winner_is_whole_and_order_independent() -> anyhow::Result<()> {
    let qso = support::id::<QsoId>(1);
    let mut left = DirectoryRecord::new(qso);
    let mut right = DirectoryRecord::new(qso);
    left.record(snapshot("K1ABC", 10, 1, Some("Earlier"))?)?;
    right.record(snapshot("K1ABC", 11, 2, None)?)?;
    right.record(snapshot("K2ABC", 12, 3, Some("Other key"))?)?;
    let mut forward = left.clone();
    forward.merge_from(&right)?;
    let mut reverse = right;
    reverse.merge_from(&left)?;
    assert_eq!(forward, reverse);
    let key = LookupKey::from_recorded(&RecordedCallsign::new("K1ABC")?);
    assert_eq!(forward.snapshot(&key).unwrap().facts().name, None);
    assert!(!forward.record(snapshot("K1ABC", 10, 1, Some("Earlier"))?)?);
    assert!(forward.record(snapshot("K1ABC", 11, 4, Some("Tie winner"))?)?);
    assert_eq!(forward.snapshot(&key).unwrap().lookup_id(), support::id(4));
    assert!(
        forward
            .record(snapshot("K1ABC", 11, 4, Some("Different same identity"))?)
            .is_err()
    );
    assert!(
        forward
            .record(snapshot(
                "K3ABC",
                11,
                4,
                Some("Different key, same identity")
            )?)
            .is_err()
    );
    assert!(
        forward
            .snapshot(&LookupKey::from_recorded(&RecordedCallsign::new("K2ABC")?))
            .is_some()
    );
    let reloaded: DirectoryRecord = serde_json::from_slice(&serde_json::to_vec(&forward)?)?;
    assert_eq!(
        reloaded.snapshot(&key).unwrap().facts().name.as_deref(),
        Some("Tie winner")
    );
    Ok(())
}

#[test]
fn directory_references_follow_whole_source_and_account_mappings() -> anyhow::Result<()> {
    use clacks_model::{
        effective::{
            EligibleStationFacts, InformationSource, StationFacts, effective_contacted_references,
            select_location,
        },
        export::{
            ExportContext,
            evaluate::{ReferenceExportInput, plan_reference_export},
            rules::{BindingCondition, FieldBinding, ValueExpression},
        },
        group::DefinitionSet,
        reference::StationSide,
    };
    let mut group = support::definitions()
        .get(support::id(1))
        .unwrap()
        .clone()
        .into_fields();
    group.name = "IOTA fixture".into();
    group.catalog = None;
    group.export_mappings.ordinary_interchange.clear();
    let unmapped = DefinitionSet::new(vec![group.clone().try_into()?])?;
    group
        .export_mappings
        .ordinary_interchange
        .push(FieldBinding {
            field: "IOTA".into(),
            value: ValueExpression::Join {
                values: Box::new(ValueExpression::Identifiers(StationSide::Contacted)),
                separator: ",".into(),
            },
            when: BindingCondition::Always,
        });
    let mapped = DefinitionSet::new(vec![group.try_into()?])?;
    let qso = support::qso(support::id(10));
    let before = qso.clone();
    let facts = StationFacts {
        references: [support::reference(support::id(1), Some("EU-005"))].into(),
        ..Default::default()
    };
    let eligible = EligibleStationFacts {
        directory: Some(&facts),
        ..Default::default()
    };
    let selected = select_location(qso.contacted_station(), &eligible, &mapped).unwrap();
    assert_eq!(selected.source, InformationSource::Directory);
    assert!(selected.value.location.is_none());
    let references =
        effective_contacted_references(qso.contacted_station(), Some(&selected), &mapped);
    let input = ReferenceExportInput {
        qso: &qso,
        contacted_references: &references,
    };
    let context = ExportContext {
        output_profile_id: support::id(1),
        target_group_id: None,
    };
    for (definitions, expected) in [(&unmapped, None), (&mapped, Some("EU-005"))] {
        let plan = plan_reference_export(
            &[input],
            &context,
            definitions,
            &support::profiles(),
            &Default::default(),
        )?;
        let fields = &plan
            .files
            .first()
            .unwrap()
            .entries
            .first()
            .unwrap()
            .reference_fields;
        assert_eq!(fields.get("IOTA").map(String::as_str), expected);
    }
    let mut partial = qso.contacted_station().clone();
    partial.location = Some(
        clacks_model::location::RecordedLocationFields {
            primary_subdivision: Some("WA".into()),
            ..Default::default()
        }
        .try_into()?,
    );
    let recorded = select_location(&partial, &eligible, &mapped).unwrap();
    assert_eq!(recorded.source, InformationSource::Recorded);
    assert!(effective_contacted_references(&partial, Some(&recorded), &mapped).is_empty());
    assert_eq!(qso, before);
    Ok(())
}
