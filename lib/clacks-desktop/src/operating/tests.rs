use super::{
    editor::{EquipmentEditor, OperatingEditor, ProfileEditor},
    station::{ReferenceForm, StationForm},
};
use clacks_client_store::{operating::OperatingState, settings::SettingsSnapshot};
use clacks_model::{
    equipment::EquipmentValues,
    group::{DefinitionSet, GroupDefinition, IdentifierDefinition, TextNormalization},
    ids::{EquipmentConfigurationId, ProfileId},
    position::{Coordinates, GridLength, Locator, Position},
    profile::{EquipmentConfiguration, SavedLocationProfile},
    reference::ReferenceAssociation,
    scalar::{Milliwatts, RecordedCallsign},
};
use clacks_sync::{
    client::{OriginalBasis, PendingRecord},
    error::UnknownReason,
    record::{Observation, RecordId, RecordKey, RecordState},
};

fn home() -> SavedLocationProfile {
    (clacks_model::profile::SavedLocationProfileFields {
        id: ProfileId::new(),
        name: "Home".into(),
        station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX").unwrap(),
            location: Some(
                (clacks_model::location::RecordedLocationFields {
                    position: Some(Position::GridArea(Locator::parse("CN87").unwrap())),
                    dxcc: Some("291".into()),
                    ..Default::default()
                })
                .try_into()
                .expect("valid native fixture"),
            ),
            references: Default::default(),
        },
        default_equipment: None,
    })
    .try_into()
    .expect("valid native fixture")
}
fn rig() -> EquipmentConfiguration {
    (clacks_model::profile::EquipmentConfigurationFields {
        id: EquipmentConfigurationId::new(),
        name: "Rig A".into(),
        values: clacks_model::equipment::EquipmentValuesFields {
            equipment_description: Some("IC-7300".into()),
            antenna_description: Some("Dipole".into()),
            notes: None,
        },
    })
    .try_into()
    .expect("valid native fixture")
}
fn active() -> OperatingState {
    let profile = home();
    let equipment = rig();
    OperatingState::new(
        Some(clacks_model::profile::CurrentOperatingValues {
            station: profile.station().clone(),
            operator: None,
            transmit_power: Some(Milliwatts::new(5000).unwrap()),
            equipment: Some(
                ((equipment.values().clone()).into_fields())
                    .try_into()
                    .expect("valid native fixture"),
            ),
        }),
        Some(profile),
        Some(equipment),
    )
    .expect("coherent operating fixture")
}

#[test]
fn manual_changes_detach_only_their_own_source_and_never_reattach() {
    let active = active();
    let mut editor = OperatingEditor::new(&active);
    let definitions = DefinitionSet::default();
    let old = editor.station.clone();
    editor.station.edit_grid("CN88".into());
    editor.station_changed(&old, &definitions);
    assert_eq!(editor.selected_profile, None);
    assert_eq!(
        editor.selected_equipment,
        active.selected_equipment().cloned()
    );
    assert_eq!(editor.expected, active);
    let old = editor.station.clone();
    editor.station.edit_grid("CN87".into());
    editor.station_changed(&old, &definitions);
    assert_eq!(editor.selected_profile, None);
    let old = editor.equipment.clone();
    editor.equipment.antenna = "Loop".into();
    editor.equipment_changed(&old);
    assert_eq!(editor.selected_equipment, None);
    assert_eq!(editor.equipment.antenna, "Loop");
}

#[test]
fn normalization_and_partial_input_do_not_corrupt_selections_or_text() {
    let active = active();
    let mut editor = OperatingEditor::new(&active);
    let defs = DefinitionSet::default();
    let old = editor.station.clone();
    editor.station.callsign = " w7hex ".into();
    editor.station_changed(&old, &defs);
    assert_eq!(editor.selected_profile, active.selected_profile().cloned());
    let old = editor.station.clone();
    editor.station.callsign.clear();
    editor.station_changed(&old, &defs);
    assert_eq!(editor.selected_profile, None);
    assert!(editor.build(&defs).is_err());
    assert_eq!(editor.station.callsign, "");
    editor.station.callsign = " w7hex/p ".into();
    assert_eq!(
        editor
            .build(&defs)
            .unwrap()
            .current()
            .unwrap()
            .station
            .callsign
            .as_str(),
        "W7HEX/P"
    );
}

#[test]
fn profile_staging_clears_absent_station_fields_and_preserves_other_scopes() {
    let active = active();
    let mut editor = OperatingEditor::new(&active);
    let mut field = home();
    {
        let mut fields = field.clone().into_fields();
        fields.station.location = None;
        field = fields.try_into().expect("valid saved fixture edit");
    }
    editor.operator_callsign = "W7ABC".into();
    editor.power_watts = "not finished".into();
    editor.stage_profile(&field, None).unwrap();
    assert_eq!(editor.station.position.build().unwrap(), None);
    assert_eq!(editor.station.dxcc, "");
    assert_eq!(
        editor.selected_equipment,
        active.selected_equipment().cloned()
    );
    assert_eq!(editor.power_watts, "not finished");
    assert_eq!(editor.operator_callsign, "W7ABC");
    let rig = rig();
    {
        let mut fields = field.clone().into_fields();
        fields.default_equipment = Some(rig.id());
        field = fields.try_into().expect("valid saved fixture edit");
    }
    let before = editor.station.clone();
    assert!(editor.stage_profile(&field, None).is_err());
    assert_eq!(editor.station, before);
    let mut wrong = rig.clone();
    {
        let mut fields = wrong.clone().into_fields();
        fields.id = EquipmentConfigurationId::new();
        wrong = fields.try_into().expect("valid saved fixture edit");
    }
    assert!(editor.stage_profile(&field, Some(&wrong)).is_err());
    assert_eq!(editor.station, before);
    editor.stage_profile(&field, Some(&rig)).unwrap();
    assert_eq!(editor.selected_equipment, Some(rig));
}

#[test]
fn power_and_independent_profile_saves_use_native_resolution() {
    let active = active();
    let mut editor = OperatingEditor::new(&active);
    let defs = DefinitionSet::default();
    for (text, expected) in [("0.001", Some(1)), ("5", Some(5000)), ("", None)] {
        editor.power_watts = text.into();
        assert_eq!(
            editor
                .build(&defs)
                .unwrap()
                .current()
                .unwrap()
                .transmit_power
                .map(|p| p.get()),
            expected
        );
        assert_eq!(editor.selected_profile, active.selected_profile().cloned());
        assert_eq!(
            editor.selected_equipment,
            active.selected_equipment().cloned()
        );
    }
    for invalid in ["0", "-1", "1.0001", "999999999999999999999999999"] {
        editor.power_watts = invalid.into();
        assert!(editor.build(&defs).is_err(), "{invalid}");
    }
    let profile = ProfileEditor::new(None, editor.station.clone());
    let mut profile = profile;
    profile.name = "Field".into();
    assert!(profile.build(&defs).is_ok());
    let mut equipment = EquipmentEditor::new(None, editor.equipment.clone());
    equipment.name = "Portable".into();
    assert!(equipment.build().is_ok());
}

#[test]
fn grid_and_point_entry_preserve_evidence_precision_and_manual_corrections() {
    let mut station = StationForm::from_snapshot(home().station());
    let defs = DefinitionSet::default();
    assert!(matches!(
        station.build(&defs).unwrap().location.unwrap().position(),
        Some(Position::GridArea(_))
    ));
    station.edit_grid("CN".into());
    assert!(station.build(&defs).is_err());
    station.edit_grid("CN87aa00aa00".into());
    assert_eq!(
        station
            .build(&defs)
            .unwrap()
            .location
            .unwrap()
            .position()
            .clone()
            .unwrap()
            .display_locator()
            .length()
            .get(),
        12
    );
    station.edit_coordinates("47.123456".into(), "-122.654321".into());
    let pos = station
        .build(&defs)
        .unwrap()
        .location
        .unwrap()
        .position()
        .clone()
        .unwrap();
    let Position::ExplicitPoint {
        coordinates,
        grid_length,
    } = pos
    else {
        panic!("point")
    };
    assert_eq!(coordinates.latitude_microdegrees(), 47_123_456);
    assert_eq!(coordinates.longitude_microdegrees(), -122_654_321);
    assert_eq!(grid_length.get(), 12);
    station.dxcc = "291".into();
    station.edit_precision(GridLength::new(6).unwrap());
    assert_eq!(station.dxcc, "291");
    station.edit_coordinates("48.123456".into(), "-122.654321".into());
    assert_eq!(station.dxcc, "");
    assert_eq!(
        Coordinates::from_decimal_degrees("48.123456", "-122.654321").unwrap(),
        station
            .position
            .build()
            .unwrap()
            .unwrap()
            .display_coordinates()
    );
}

#[test]
fn existing_references_survive_definition_changes_but_explicit_edits_use_rules() {
    let group = clacks_model::ids::GroupId::new();
    let association = ReferenceAssociation::new(group, Some("old-value".into()))
        .expect("valid reference fixture");
    let mut native = home().station().clone();
    native.references.insert(association.clone());
    let mut form = StationForm::from_snapshot(&native);
    let definition = GroupDefinition {
        id: group,
        name: "Event".into(),
        identifier: IdentifierDefinition {
            required: true,
            normalization: vec![TextNormalization::Trim, TextNormalization::Uppercase],
            syntax: Some("US-[0-9]+".into()),
        },
        bears_location_context: true,
        catalog: None,
        export_mappings: Default::default(),
    };
    let defs = DefinitionSet::new(vec![definition]).unwrap();
    assert_eq!(form.build(&defs).unwrap().references, native.references);
    form.references = vec![ReferenceForm::new()];
    let row = form.references.first_mut().unwrap();
    row.group = Some(group);
    row.identifier = " us-0001 ".into();
    row.edited = true;
    assert_eq!(
        form.build(&defs)
            .unwrap()
            .references
            .first()
            .unwrap()
            .identifier()
            .as_deref(),
        Some("US-0001")
    );
    form.references.first_mut().unwrap().identifier.clear();
    assert!(form.build(&defs).is_err());
}

#[test]
fn deletion_refresh_clears_sources_without_losing_pending_values_or_rebasing_real_edits() {
    let active = active();
    let mut editor = OperatingEditor::new(&active);
    editor.power_watts = "25".into();
    let profile = active.selected_profile().unwrap();
    let key = RecordKey {
        account: clacks_model::ids::AccountId::new(),
        id: RecordId::Profile(profile.id()),
    };
    let row = clacks_sync::client::ClientRecordFields {
        key,
        latest: Observation::Unavailable {
            key,
            reason: UnknownReason::MissingOriginal,
        },
        pending: Some(PendingRecord {
            key,
            original: OriginalBasis::Unavailable {
                key,
                reason: UnknownReason::MissingOriginal,
            },
            working: RecordState::Deleted,
            prepared: None,
            witness: None,
        }),
        retained: vec![],
    }
    .try_into()
    .expect("coherent client fixture");
    let mut current = active.clone();
    current.forget_source(RecordId::Profile(home().id()));
    let view = SettingsSnapshot {
        logbooks: clacks_client_store::logbooks::LogbookSnapshot {
            selected_view: clacks_client_store::logbooks::selection::SelectedView::AllContacts,
            tags: vec![],
        },
        operating: current.clone(),
        profiles: vec![row],
        equipment: vec![],
        groups: vec![],
    };
    editor.refresh_sources(&view);
    assert_eq!(editor.selected_profile, None);
    assert_eq!(editor.power_watts, "25");
    assert_eq!(editor.expected, current);
    let mut editor = OperatingEditor::new(&active);
    let mut changed = view.clone();
    let mut values = changed.operating.current().cloned().unwrap();
    values.transmit_power = Some(Milliwatts::new(10_000).unwrap());
    changed.operating.adjust(values);
    editor.refresh_sources(&changed);
    assert_eq!(editor.selected_profile, None);
    assert_eq!(editor.expected.current(), active.current());
}

#[test]
fn first_coordinate_entry_uses_six_characters_without_quantizing_the_point() {
    let mut station = StationForm::empty();
    station.callsign = "W7HEX".into();
    station.edit_coordinates("47.123456".into(), "-122.654321".into());
    let position = station
        .build(&DefinitionSet::default())
        .unwrap()
        .location
        .unwrap()
        .position()
        .clone()
        .unwrap();
    assert_eq!(position.display_locator().length().get(), 6);
    assert_eq!(
        position.display_coordinates().latitude_microdegrees(),
        47_123_456
    );
}

#[test]
fn selecting_a_named_configuration_with_no_descriptions_remains_valid() {
    let mut rig = rig();
    {
        let mut fields = rig.clone().into_fields();
        fields.values = EquipmentValues::default().into_fields();
        rig = fields.try_into().expect("valid saved fixture edit");
    }
    let mut editor = OperatingEditor::new(&active());
    editor.stage_equipment(&rig);
    let built = editor.build(&DefinitionSet::default()).unwrap();
    assert_eq!(
        built.current().unwrap().equipment,
        Some(EquipmentValues::default())
    );
    assert_eq!(built.selected_equipment().unwrap().id(), rig.id());
}

#[test]
fn partial_grid_does_not_reduce_valid_coordinate_entry_precision() {
    let mut station = StationForm::empty();
    station.callsign = "W7HEX".into();
    station.edit_grid("CN".into());
    assert_eq!(station.position.grid, "CN");
    station.edit_coordinates("47.".into(), String::new());
    assert_eq!(station.position.latitude, "47.");
    assert_eq!(station.position.longitude, "");
    assert!(station.build(&DefinitionSet::default()).is_err());
    station.edit_coordinates("47.123456".into(), "-122.654321".into());
    assert_eq!(station.position.length, GridLength::default());
    let built = station.build(&DefinitionSet::default()).unwrap();
    assert!(
        matches!(built.location.unwrap().position(), Some(Position::ExplicitPoint { coordinates, grid_length }) if *coordinates == Coordinates::from_decimal_degrees("47.123456", "-122.654321").unwrap() && *grid_length == GridLength::default())
    );
}
