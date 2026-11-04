use super::super::{contact, protocol};
use super::protocol::adif;
use clacks_model::ids::QsoId;

#[test]
fn supplied_power_truncates_to_milliwatts() -> anyhow::Result<()> {
    for (text, want) in [("5.0000", 5000), ("5.0007", 5000), ("0.0019", 1)] {
        assert_eq!(contact::parse_power_watts(text)?.unwrap().get(), want);
    }
    assert!(contact::parse_power_watts("0.0007").is_err());
    assert!(contact::parse_power_watts("9223372036854776").is_err());
    Ok(())
}

#[test]
fn live_context_and_selected_tag() -> anyhow::Result<()> {
    let notice=protocol::decode(&adif(b"<CALL:5>W7HEX<QSO_DATE:8>20261103<TIME_ON:6>123456<FREQ:6>14.074<MODE:3>FT8<STATION_CALLSIGN:5>W7ABC<TX_PWR:6>5.0007<EOR>"),"127.0.0.1:1".parse()?)?.unwrap();
    let mut context = super::context()?;
    context.selected_view = clacks_client_store::logbooks::selection::SelectedView::Logbook(
        "00000000000000000000000021".parse()?,
    );
    let mut active = context.operating.current().cloned().unwrap();
    active.transmit_power = Some(clacks_model::scalar::Milliwatts::new(1000)?);
    active.station.location = Some(
        clacks_model::location::RecordedLocationFields {
            primary_subdivision: Some("WA".into()),
            ..Default::default()
        }
        .try_into()?,
    );
    context.operating.adjust(active);
    let projection = contact::build(QsoId::new(), &notice, &context)?;
    assert_eq!(projection.qso.own_station().callsign.as_str(), "W7ABC");
    assert_eq!(
        projection.qso.own_context().transmit_power.unwrap().get(),
        5000
    );
    assert_eq!(projection.qso.contacted_station().location, None);
    assert!(
        projection
            .qso
            .tags()
            .contains(&"00000000000000000000000021".parse()?)
    );
    assert_eq!(
        projection
            .qso
            .own_station()
            .location
            .as_ref()
            .unwrap()
            .primary_subdivision()
            .as_deref(),
        Some("WA")
    );
    let mut absent = notice.clone();
    absent.facts.values.remove("STATION_CALLSIGN");
    assert_eq!(
        contact::build(QsoId::new(), &absent, &context)?
            .qso
            .own_station()
            .callsign
            .as_str(),
        "W7ZZZ"
    );
    absent
        .facts
        .values
        .insert("STATION_CALLSIGN".into(), String::new());
    absent.facts.values.insert("TX_PWR".into(), "0.0007".into());
    let omitted = contact::build(QsoId::new(), &absent, &context)?;
    assert_eq!(omitted.qso.own_context().transmit_power, None);
    assert!(omitted.warnings.iter().any(|field| field == "TX_PWR"));

    Ok(())
}

#[test]
fn contest_lookup_respects_binding_conditions() -> anyhow::Result<()> {
    use clacks_model::{
        export::rules::{BindingCondition, FieldBinding, GroupExportMappings, ValueExpression},
        group::{DefinitionSet, GroupDefinitionFields, IdentifierDefinition},
        reference::StationSide,
    };
    let definition = |id: &str, conditions: Vec<BindingCondition>| -> anyhow::Result<_> {
        Ok(GroupDefinitionFields {
            id: id.parse()?,
            name: "Contest".into(),
            identifier: IdentifierDefinition {
                required: false,
                normalization: vec![],
                syntax: None,
            },
            bears_location_context: false,
            catalog: None,
            export_mappings: GroupExportMappings {
                ordinary_interchange: conditions
                    .into_iter()
                    .map(|when| FieldBinding {
                        field: "CONTEST_ID".into(),
                        value: ValueExpression::Literal("ARRL-FIELD-DAY".into()),
                        when,
                    })
                    .collect(),
                targeted: Default::default(),
            },
        }
        .try_into()?)
    };
    let own = definition(
        "00000000000000000000000011",
        vec![
            BindingCondition::AssociationPresent(StationSide::Own),
            BindingCondition::Always,
        ],
    )?;
    let contacted = definition(
        "00000000000000000000000012",
        vec![BindingCondition::AssociationPresent(StationSide::Contacted)],
    )?;
    let notice=protocol::decode(&adif(b"<CALL:5>W7HEX<QSO_DATE:8>20261103<TIME_ON:6>123456<FREQ:6>14.074<MODE:3>FT8<STATION_CALLSIGN:5>W7ABC<CONTEST_ID:14>ARRL-FIELD-DAY<CLASS:2>1D<ARRL_SECT:3>WWA<EOR>"),"127.0.0.1:1".parse()?)?.unwrap();
    let mut context = super::context()?;
    context.definitions = DefinitionSet::new(vec![own, contacted])?;
    let projection = contact::build(QsoId::new(), &notice, &context)?;
    assert_eq!(
        projection
            .qso
            .own_station()
            .references
            .iter()
            .next()
            .unwrap()
            .group_id
            .to_string(),
        "00000000000000000000000011"
    );
    assert_eq!(projection.qso.own_station().location, None);
    context.definitions = DefinitionSet::new(vec![
        definition("00000000000000000000000011", vec![BindingCondition::Always])?,
        definition("00000000000000000000000013", vec![BindingCondition::Always])?,
    ])?;
    let projection = contact::build(QsoId::new(), &notice, &context)?;
    assert!(projection.qso.own_station().references.is_empty());
    assert!(
        projection
            .warnings
            .iter()
            .any(|field| field == "CONTEST_ID")
    );
    Ok(())
}
