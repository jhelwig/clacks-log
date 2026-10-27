use super::time::ContactTimestamp;
use clacks_model::scalar::UtcSecond;

#[test]
fn independent_clocks_preserve_unknown_and_overnight_end() -> anyhow::Result<()> {
    let before = UtcSecond::from_unix_seconds(1_792_967_999);
    let after = UtcSecond::from_unix_seconds(1_792_968_001);
    let mut start = ContactTimestamp::live(before);
    let mut end = ContactTimestamp::live(before);
    start.lock();
    end.tick(after)?;
    assert_eq!(
        start.value()?.map(UtcSecond::unix_seconds),
        Some(before.unix_seconds())
    );
    assert_eq!(
        end.value()?.map(UtcSecond::unix_seconds),
        Some(after.unix_seconds())
    );
    end.lock();
    end.text.clear();
    end.tick(after)?;
    assert_eq!(end.value()?, None);
    end.unlock(after)?;
    assert_eq!(end.value()?, Some(after));
    Ok(())
}

#[test]
fn complete_contact_form_builds_native_facts() -> anyhow::Result<()> {
    use super::form::ContactForm;
    use clacks_model::{group::DefinitionSet, ids::QsoId};
    let mut form = ContactForm::empty();
    form.own.callsign = "w7hex".into();
    form.dx.callsign = "ok2cqr/p".into();
    form.dx_name = "Petr".into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start = ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(10)));
    form.end = ContactTimestamp::recorded(None);
    form.power = ".5".into();
    let qso = form
        .build(QsoId::new(), &DefinitionSet::new(vec![])?)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(qso.own_station.callsign.as_str(), "W7HEX");
    assert_eq!(qso.contacted_station.callsign.as_str(), "OK2CQR/P");
    assert_eq!(
        qso.contacted_operator
            .as_ref()
            .and_then(|v| v.recorded_name.as_deref()),
        Some("Petr")
    );
    assert_eq!(qso.own_context.transmit_power.map(|v| v.get()), Some(500));
    assert_eq!(qso.end, None);
    Ok(())
}

#[test]
fn unrelated_edit_preserves_unrecognized_and_coarse_values() -> anyhow::Result<()> {
    use super::form::ContactForm;
    use clacks_model::{
        group::DefinitionSet,
        ids::QsoId,
        position::{Locator, Position},
    };
    let mut form = ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "OK2CQR".into();
    form.primary.band = "unknown-band".into();
    form.mode = "unfamiliar-mode".into();
    form.start = ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(1)));
    form.end = ContactTimestamp::recorded(None);
    let defs = DefinitionSet::new(vec![])?;
    let mut qso = form
        .build(QsoId::new(), &defs)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    qso.own_station.location = Some(clacks_model::location::RecordedLocation {
        position: Some(Position::GridArea(Locator::parse("AA")?)),
        ..Default::default()
    });
    qso.own_context.equipment = Some(clacks_model::equipment::EquipmentValues::default());
    qso.own_station
        .references
        .insert(clacks_model::reference::ReferenceAssociation {
            group_id: clacks_model::ids::GroupId::new(),
            identifier: Some("unfamiliar-reference".into()),
        });
    qso.mode = clacks_model::recognition::mode::Mode::from_recorded_identity(
        clacks_model::recognition::mode::ModeIdentity::Specialization {
            label: "unfamiliar-submode".into(),
            parent: Some("unfamiliar-parent".into()),
        },
    )?;
    let mut edit = ContactForm::from_qso(&qso);
    edit.comment = "changed".into();
    let next = edit
        .build(qso.id, &defs)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(next.own_station, qso.own_station);
    assert_eq!(next.mode, qso.mode);
    assert_eq!(next.own_context, qso.own_context);
    assert_eq!(next.primary, qso.primary);
    Ok(())
}

#[test]
fn active_defaults_preserve_explicit_corrections_and_clears() -> anyhow::Result<()> {
    use super::defaults::NewEntry;
    use clacks_client_store::{logbooks::selection::SelectedView, operating::OperatingState};
    let mut entry = NewEntry::new(
        &OperatingState::default(),
        SelectedView::AllContacts,
        UtcSecond::from_unix_seconds(1),
    );
    let before = entry.form.clone();
    entry.form.power = ".5".into();
    entry.capture_corrections(&before);
    entry.refresh_defaults(&OperatingState::default(), SelectedView::Untagged);
    assert_eq!(entry.form.power, ".5");
    assert!(entry.form.tags.is_empty());
    Ok(())
}

#[test]
fn view_switch_replaces_only_proposed_membership() {
    use super::defaults::NewEntry;
    use clacks_client_store::{logbooks::selection::SelectedView, operating::OperatingState};
    use clacks_model::ids::TagId;
    let first = TagId::new();
    let next = TagId::new();
    let mut entry = NewEntry::new(
        &OperatingState::default(),
        SelectedView::Logbook(first),
        UtcSecond::from_unix_seconds(1),
    );
    entry.form.dx.callsign = "OK2CQR".into();
    entry.refresh_defaults(&OperatingState::default(), SelectedView::Logbook(next));
    assert_eq!(entry.form.tags, [next].into());
    entry.refresh_defaults(&OperatingState::default(), SelectedView::AllContacts);
    assert!(entry.form.tags.is_empty());
    assert_eq!(entry.form.dx.callsign, "OK2CQR");
}

#[test]
fn active_power_follows_defaults_until_explicitly_corrected() -> anyhow::Result<()> {
    use super::defaults::NewEntry;
    use clacks_client_store::{logbooks::selection::SelectedView, operating::OperatingState};
    use clacks_model::{
        contact::StationSnapshot,
        profile::CurrentOperatingValues,
        scalar::{Milliwatts, RecordedCallsign},
    };
    let mut active = OperatingState {
        current: Some(CurrentOperatingValues {
            station: StationSnapshot {
                callsign: RecordedCallsign::new("W7HEX")?,
                location: None,
                references: Default::default(),
            },
            operator: None,
            transmit_power: Some(Milliwatts::new(100_000)?),
            equipment: None,
        }),
        ..Default::default()
    };
    let mut entry = NewEntry::new(
        &active,
        SelectedView::AllContacts,
        UtcSecond::from_unix_seconds(1),
    );
    assert_eq!(entry.form.power, "100");
    active.current.as_mut().unwrap().transmit_power = Some(Milliwatts::new(50_000)?);
    entry.refresh_defaults(&active, SelectedView::AllContacts);
    assert_eq!(entry.form.power, "50");
    let before = entry.form.clone();
    entry.form.power.clear();
    entry.capture_corrections(&before);
    active.current.as_mut().unwrap().transmit_power = Some(Milliwatts::new(25_000)?);
    entry.refresh_defaults(&active, SelectedView::AllContacts);
    assert!(entry.form.power.is_empty());
    Ok(())
}
