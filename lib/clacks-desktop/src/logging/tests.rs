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
    for (seconds, text) in [
        (-1, "1969-12-31 23:59:59"),
        (253_402_300_799, "9999-12-31 23:59:59"),
    ] {
        let value = UtcSecond::from_unix_seconds(seconds);
        let timestamp = ContactTimestamp::recorded(Some(value));
        assert_eq!(timestamp.text, text);
        assert_eq!(timestamp.value()?, Some(value));
    }
    assert_eq!(
        ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(i64::MAX))).text,
        i64::MAX.to_string()
    );
    for text in ["2026-10-27 23:59:60", "2026-1-2 03:04:05"] {
        start.text = text.into();
        assert!(start.value().is_err());
    }
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
    assert_eq!(qso.own_station().callsign.as_str(), "W7HEX");
    assert_eq!(qso.contacted_station().callsign.as_str(), "OK2CQR/P");
    assert_eq!(
        qso.contacted_operator()
            .as_ref()
            .and_then(|v| v.recorded_name().as_deref()),
        Some("Petr")
    );
    assert_eq!(qso.own_context().transmit_power.map(|v| v.get()), Some(500));
    assert_eq!(qso.end(), None);
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
    crate::test_support::edit(&mut qso, |qso| {
        qso.own_station.location = Some(
            (clacks_model::location::RecordedLocationFields {
                position: Some(Position::GridArea(Locator::parse("AA")?)),
                dxcc: Some("291".into()),
                primary_subdivision: Some("WA".into()),
                secondary_subdivision: Some("Manual county correction".into()),
                cq_zone: Some(clacks_model::location::CqZone::new(3)?),
                itu_zone: Some(clacks_model::location::ItuZone::new(6)?),
            })
            .try_into()
            .expect("valid native fixture"),
        );
        qso.contacted_station.location = qso.own_station.location.clone();
        Ok(())
    });
    crate::test_support::edit(&mut qso, |qso| {
        qso.own_context.equipment = Some(
            ((clacks_model::equipment::EquipmentValues::default()).into_fields())
                .try_into()
                .expect("valid native fixture"),
        );
        Ok(())
    });
    crate::test_support::edit(&mut qso, |qso| {
        qso.own_station.references.insert(
            clacks_model::reference::ReferenceAssociation::new(
                clacks_model::ids::GroupId::new(),
                Some("unfamiliar-reference".into()),
            )
            .expect("valid reference fixture"),
        );
        Ok(())
    });
    crate::test_support::edit(&mut qso, |qso| {
        qso.mode = clacks_model::recognition::mode::Mode::from_recorded_identity(
            clacks_model::recognition::mode::ModeIdentity::Specialization {
                label: "unfamiliar-submode".into(),
                parent: Some("unfamiliar-parent".into()),
            },
        )?;
        Ok(())
    });
    let mut edit = ContactForm::from_qso(&qso);
    assert!(!edit.own.has_geographic_intent());
    assert!(!edit.dx.has_geographic_intent());
    edit.comment = "changed".into();
    let next = edit
        .build(qso.id(), &defs)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(next.own_station(), qso.own_station());
    assert_eq!(next.contacted_station(), qso.contacted_station());
    assert_eq!(next.mode(), qso.mode());
    assert_eq!(next.own_context(), qso.own_context());
    assert_eq!(next.primary(), qso.primary());
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
    use clacks_model::scalar::{Milliwatts, RecordedCallsign};
    let mut active = OperatingState::new(
        Some(clacks_model::profile::CurrentOperatingValues {
            station: clacks_model::contact::StationSnapshot {
                callsign: RecordedCallsign::new("W7HEX")?,
                location: None,
                references: Default::default(),
            },
            operator: None,
            transmit_power: Some(Milliwatts::new(100_000)?),
            equipment: None,
        }),
        None,
        None,
    )
    .expect("coherent operating fixture");
    let mut entry = NewEntry::new(
        &active,
        SelectedView::AllContacts,
        UtcSecond::from_unix_seconds(1),
    );
    assert_eq!(entry.form.power, "100");
    let mut values = active.current().cloned().unwrap();
    values.transmit_power = Some(Milliwatts::new(50_000)?);
    active.adjust(values);
    entry.refresh_defaults(&active, SelectedView::AllContacts);
    assert_eq!(entry.form.power, "50");
    let before = entry.form.clone();
    entry.form.power.clear();
    entry.capture_corrections(&before);
    let mut values = active.current().cloned().unwrap();
    values.transmit_power = Some(Milliwatts::new(25_000)?);
    active.adjust(values);
    entry.refresh_defaults(&active, SelectedView::AllContacts);
    assert!(entry.form.power.is_empty());
    Ok(())
}
