mod support;

use clacks_model::contact::OperatorInformation;
use clacks_model::equipment::EquipmentValues;
use clacks_model::ids::{EquipmentConfigurationId, GroupId, ProfileId, QsoId, TagId};
use clacks_model::location::RecordedLocation;
use clacks_model::profile::{
    CurrentOperatingValues, EquipmentConfiguration, SavedLocationProfile, load_equipment,
    load_profile,
};
use clacks_model::scalar::Milliwatts;
use clacks_model::tag::{
    AutomaticUploadPolicy, LogbookTag, TagDeletionChoice, automatic_upload_allowed,
    plan_tag_deletion,
};
fn current() -> CurrentOperatingValues {
    let q = support::qso(support::id::<QsoId>(1));
    CurrentOperatingValues {
        station: q.own_station,
        operator: Some(OperatorInformation {
            callsign: None,
            recorded_name: Some("Operator".into()),
        }),
        transmit_power: Some(Milliwatts::new(5000).unwrap()),
        equipment: Some(EquipmentValues {
            antenna_description: Some("dipole".into()),
            ..Default::default()
        }),
    }
}

fn profile() -> SavedLocationProfile {
    SavedLocationProfile {
        id: support::id::<ProfileId>(1),
        name: "field".into(),
        station: support::qso(support::id::<QsoId>(1)).own_station,
        default_equipment: None,
    }
}

#[test]
fn profile_load_clears_old_station_and_preserves_nonprofile_values() {
    let mut c = current();
    c.station.location = Some(RecordedLocation {
        secondary_subdivision: Some("county".into()),
        ..Default::default()
    });
    c.station.references.insert(support::reference(
        support::id::<GroupId>(1),
        Some("US-1234"),
    ));
    let loaded = load_profile(&c, &profile(), None).unwrap();
    assert_eq!(loaded.station, profile().station);
    assert_eq!(loaded.operator, c.operator);
    assert_eq!(loaded.transmit_power, c.transmit_power);
    assert_eq!(loaded.equipment, c.equipment);
    let mut p = profile();
    p.default_equipment = Some(support::id::<EquipmentConfigurationId>(1));
    assert!(load_profile(&c, &p, None).is_err());
    let wrong = EquipmentConfiguration {
        id: support::id::<EquipmentConfigurationId>(2),
        name: "wrong".into(),
        values: EquipmentValues::default(),
    };
    assert!(load_profile(&c, &p, Some(&wrong)).is_err());
}

#[test]
fn equipment_copies_survive_configuration_edit() {
    let mut e = EquipmentConfiguration {
        id: support::id::<EquipmentConfigurationId>(1),
        name: "base".into(),
        values: EquipmentValues {
            equipment_description: Some("rig A".into()),
            ..Default::default()
        },
    };
    let mut p = profile();
    p.default_equipment = Some(e.id);
    let c = load_profile(&current(), &p, Some(&e)).unwrap();
    let mut q = support::qso(support::id::<QsoId>(1));
    q.own_context.equipment = c.equipment.clone();
    e.values.equipment_description = Some("rig B".into());
    assert_eq!(
        q.own_context
            .equipment
            .as_ref()
            .unwrap()
            .equipment_description
            .as_deref(),
        Some("rig A")
    );
    assert_eq!(c.equipment, q.own_context.equipment);
    assert_ne!(load_equipment(&c, &e).equipment, c.equipment);
}

#[test]
fn upload_policy_truth_table() {
    use AutomaticUploadPolicy::*;
    assert!(!automatic_upload_allowed([]));
    for a in [Neither, Enable, Forbid] {
        for b in [Neither, Enable, Forbid] {
            let expected = a != Forbid && b != Forbid && (a == Enable || b == Enable);
            assert_eq!(automatic_upload_allowed([a, b]), expected);
        }
    }
    let t = LogbookTag::new(support::id::<TagId>(1), "test".into());
    assert_eq!(t.automatic_upload.lotw, Neither);
    assert_eq!(t.automatic_upload.qrz, Neither);
}

#[test]
fn tag_deletion_counts_identities() {
    let tag = support::id::<TagId>(1);
    let mut qsos: Vec<_> = (1..=100)
        .map(|n| {
            let mut q = support::qso(support::id::<QsoId>(n));
            q.tags.insert(tag);
            if n <= 30 {
                q.tags.insert(support::id::<TagId>(2));
            }
            q
        })
        .collect();
    qsos.push(qsos[0].clone());
    for (choice, deleted, untagged) in [
        (TagDeletionChoice::LeaveAll, 0, 100),
        (TagDeletionChoice::DeleteAll, 100, 0),
        (TagDeletionChoice::DeleteUnique, 70, 30),
    ] {
        let plan = plan_tag_deletion(tag, choice, &qsos);
        assert_eq!(plan.affected.len(), 100);
        assert_eq!(plan.also_tagged_elsewhere, 30);
        assert_eq!(plan.mark_deleted.len(), deleted);
        assert_eq!(plan.remove_membership.len(), untagged);
        assert!(plan.mark_deleted.is_disjoint(&plan.remove_membership));
    }
    assert!(qsos.iter().all(|q| q.tags.contains(&tag)));
}
