//! Native input snapshots for component tests; no application services are started.

use clacks_client_store::{
    logbooks::{LogbookSnapshot, selection::SelectedView},
    operating::OperatingState,
    settings::SettingsSnapshot,
};
use clacks_model::{ids::AccountId, scalar::RecordedCallsign};
use clacks_sync::{
    client::{ClientRecord, OriginalBasis, PendingRecord},
    error::UnknownReason,
    record::{Observation, RecordKey, RecordState, RecordValue},
};

pub(in crate::ui) fn row(value: RecordValue) -> ClientRecord {
    let key = RecordKey {
        account: "00000000000000000000000001".parse::<AccountId>().unwrap(),
        id: value.id(),
    };
    clacks_sync::client::ClientRecordFields {
        key,
        latest: Observation::Unavailable {
            key,
            reason: UnknownReason::MissingOriginal,
        },
        retained: vec![],
        pending: Some(PendingRecord {
            key,
            original: OriginalBasis::Unavailable {
                key,
                reason: UnknownReason::MissingOriginal,
            },
            working: RecordState::Live(value),
            prepared: None,
            witness: None,
        }),
    }
    .try_into()
    .expect("coherent client fixture")
}

pub(in crate::ui) fn snapshot() -> SettingsSnapshot {
    let equipment = clacks_model::profile::EquipmentConfigurationFields {
        id: Default::default(),
        name: "Rig A".into(),
        values: clacks_model::equipment::EquipmentValuesFields {
            equipment_description: Some("IC-7300".into()),
            ..Default::default()
        },
    };
    let home = clacks_model::profile::SavedLocationProfileFields {
        id: Default::default(),
        name: "Home".into(),
        station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX").unwrap(),
            location: None,
            references: Default::default(),
        },
        default_equipment: Some(equipment.id),
    };
    SettingsSnapshot {
        logbooks: LogbookSnapshot {
            selected_view: SelectedView::AllContacts,
            tags: vec![],
        },
        operating: OperatingState::new(
            Some(clacks_model::profile::CurrentOperatingValues {
                station: home.station.clone(),
                equipment: Some(
                    (equipment.values.clone())
                        .try_into()
                        .expect("valid native fixture"),
                ),
                operator: None,
                transmit_power: None,
            }),
            Some((home.clone()).try_into().expect("valid native fixture")),
            Some(
                (equipment.clone())
                    .try_into()
                    .expect("valid native fixture"),
            ),
        )
        .expect("coherent operating fixture"),
        profiles: vec![row(RecordValue::Profile(Box::new(
            (home).try_into().expect("valid native fixture"),
        )))],
        equipment: vec![row(RecordValue::Equipment(Box::new(
            (equipment).try_into().expect("valid native fixture"),
        )))],
        groups: vec![],
    }
}

pub(in crate::ui) fn contact() -> clacks_model::contact::Qso {
    use crate::logging::{form::ContactForm, time::ContactTimestamp};
    use clacks_model::{group::DefinitionSet, scalar::UtcSecond};
    let mut form = ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "OK2CQR".into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start = ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(1)));
    form.build(Default::default(), &DefinitionSet::default())
        .unwrap()
}

/// Ordered component input with distinct callsigns and native contact identities.
pub(in crate::ui) fn contact_page(
    start: u32,
    count: u32,
    total: u64,
) -> clacks_client_store::read::contacts::ContactPage {
    use clacks_client_store::read::{
        contacts::{ContactPage, ContactRow},
        history::HistoryRow,
    };
    ContactPage {
        rows: (start..start + count)
            .map(|index| {
                let mut qso = contact();
                crate::test_support::edit(&mut qso, |qso| {
                    qso.contacted_station.callsign =
                        RecordedCallsign::new(&format!("K1T{index:03}")).unwrap();
                    Ok(())
                });
                ContactRow {
                    facts: Default::default(),
                    working: HistoryRow {
                        qso,
                        pending: true,
                        review: None,
                    },
                }
            })
            .collect(),
        matching_total: total,
        selected_total: 0,
        selected_outside_filter: 0,
        has_more: u64::from(start + count) < total,
    }
}
