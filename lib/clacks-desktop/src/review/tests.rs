use super::{
    comparison::{ComparisonBasis, ComparisonSource},
    draft::ResolutionDraft,
    fields::{FieldId, QsoField, StationField},
    input::FieldInput,
};
use clacks_model::{
    contact::Qso,
    group::DefinitionSet,
    ids::TagId,
    position::{Locator, Position},
    recognition::{
        band::Band,
        mode::{Mode, ModeIdentity},
    },
    scalar::{RecordedCallsign, UtcSecond},
};
use clacks_sync::{
    client::{OriginalBasis, PendingRecord, ReviewInput, ReviewReason},
    clock::{ClockActor, RecordVector},
    record::{
        AcceptedRecord, Observation, Position as HistoryPosition, RecordId, RecordKey, RecordState,
        RecordValue, VersionStamp,
    },
};

pub(crate) fn basis() -> ComparisonBasis {
    let mut form = crate::logging::form::ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "OK2CQR".into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start =
        crate::logging::time::ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(1)));
    let original = form
        .build(Default::default(), &DefinitionSet::default())
        .unwrap();
    let mut local = original.clone().into_fields();
    local.comment = Some("Portable".into());
    local.tags = [TagId::new(), TagId::new()].into();
    let mut shared = original.clone().into_fields();
    shared.comment = Some("Rain".into());
    shared.tags = [*local.tags.first().unwrap(), TagId::new()].into();
    let key = RecordKey {
        account: "00000000000000000000000001".parse().unwrap(),
        id: RecordId::Qso(original.id()),
    };
    let timeline = Default::default();
    let baseline = Default::default();
    let accepted = |revision, qso: Qso| {
        AcceptedRecord::new(
            key,
            RecordState::Live(RecordValue::Qso(Box::new(qso))),
            VersionStamp::new(
                HistoryPosition { timeline, revision },
                baseline,
                RecordVector::from_entries([(ClockActor::Server(timeline), revision)]).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let original = OriginalBasis::Accepted(accepted(1, original));
    let shared = Observation::Accepted(accepted(2, shared.try_into().unwrap()));
    let local = RecordState::Live(RecordValue::Qso(Box::new(local.try_into().unwrap())));
    let review = ReviewInput {
        original: original.clone(),
        local: local.clone(),
        shared: shared.clone(),
        reason: ReviewReason::BothDifferent,
        retained: vec![],
    };
    let record = clacks_sync::client::ClientRecordFields {
        key,
        latest: shared,
        retained: vec![],
        pending: Some(PendingRecord {
            key,
            original,
            working: local,
            prepared: None,
            witness: None,
        }),
    }
    .try_into()
    .unwrap();
    ComparisonBasis { record, review }
}

fn qso(state: &RecordState) -> &Qso {
    let RecordState::Live(RecordValue::Qso(qso)) = state else {
        panic!("live QSO")
    };
    qso
}

#[test]
fn resolution_choices_are_explicit_and_membership_starts_local() -> anyhow::Result<()> {
    let basis = basis();
    let local = qso(&basis.review.local).tags().clone();
    let Observation::Accepted(shared) = &basis.review.shared else {
        panic!("shared")
    };
    let shared_tags = qso(shared.state()).tags().clone();
    let mut draft = ResolutionDraft::new(basis)?;
    assert!(draft.build(&DefinitionSet::default()).is_err());
    assert_eq!(draft.membership(), Some(&local));
    draft.set_custom(
        FieldId::Qso(QsoField::Comment),
        FieldInput::Text {
            value: String::new(),
        },
    );
    let built = draft.build(&DefinitionSet::default()).unwrap();
    assert_eq!(qso(&built).comment(), &None);
    assert_eq!(qso(&built).tags(), &local);
    draft.select(FieldId::Qso(QsoField::Tags), ComparisonSource::Shared)?;
    let all = shared_tags.union(&local).copied().collect();
    draft.set_custom(
        FieldId::Qso(QsoField::Tags),
        FieldInput::Membership { tags: all },
    );
    let built = draft.build(&DefinitionSet::default()).unwrap();
    assert_eq!(qso(&built).tags().len(), 3);
    draft.set_custom(
        FieldId::Qso(QsoField::Tags),
        FieldInput::Membership {
            tags: Default::default(),
        },
    );
    assert!(
        qso(&draft.build(&DefinitionSet::default()).unwrap())
            .tags()
            .is_empty()
    );
    Ok(())
}

#[test]
fn resolution_preserves_selected_native_values() -> anyhow::Result<()> {
    let mut basis = basis();
    let Observation::Accepted(shared) = &mut basis.review.shared else {
        panic!("shared")
    };
    let mut values = qso(shared.state()).clone().into_fields();
    values.own_station.location = Some(
        clacks_model::location::RecordedLocationFields {
            position: Some(Position::GridArea(Locator::parse("CN")?)),
            ..Default::default()
        }
        .try_into()?,
    );
    values.mode = Mode::from_recorded_identity(ModeIdentity::Specialization {
        label: "future-mode".into(),
        parent: Some("future-parent".into()),
    })?;
    values.primary = clacks_model::contact::BandFrequency::from_recorded_pair(
        None,
        Some(Band::from_recorded_label("future-band")?),
    )?;
    values.start = UtcSecond::from_unix_seconds(i64::MIN / 2);
    *shared = AcceptedRecord::new(
        shared.key(),
        RecordState::Live(RecordValue::Qso(Box::new(values.try_into()?))),
        shared.stamp.clone(),
    )?;
    let expected = qso(shared.state()).clone();
    sync_record(&mut basis);
    let mut draft = ResolutionDraft::new(basis)?;
    draft.select_all(ComparisonSource::Shared)?;
    draft.set_custom(
        FieldId::Qso(QsoField::Comment),
        FieldInput::Text {
            value: "New wording".into(),
        },
    );
    let built = draft.build(&DefinitionSet::default()).unwrap();
    assert_eq!(qso(&built).own_station(), expected.own_station());
    assert_eq!(qso(&built).mode(), expected.mode());
    assert_eq!(qso(&built).primary(), expected.primary());
    assert_eq!(qso(&built).start(), expected.start());
    draft.set_custom(
        FieldId::Qso(QsoField::Station {
            side: clacks_model::reference::StationSide::Own,
            field: StationField::Position,
        }),
        FieldInput::Position {
            form: crate::operating::position::PositionForm::from_position(Some(
                &Position::GridArea(Locator::parse("CN")?),
            )),
        },
    );
    assert!(draft.build(&DefinitionSet::default()).is_err());
    Ok(())
}

#[test]
fn resolution_draft_refresh_preserves_inputs() -> anyhow::Result<()> {
    let basis = basis();
    let mut draft = ResolutionDraft::new(basis.clone())?;
    draft.select_all(ComparisonSource::Local)?;
    draft.set_custom(
        FieldId::Qso(QsoField::Comment),
        FieldInput::Text {
            value: "My combined comment".into(),
        },
    );
    draft.observe(&basis);
    assert!(!draft.is_stale());
    let mut newer = basis.clone();
    let Observation::Accepted(shared) = &mut newer.review.shared else {
        panic!("shared")
    };
    let mut values = qso(shared.state()).clone().into_fields();
    values.comment = Some("Updated remotely".into());
    values.contacted_station.callsign = RecordedCallsign::new("OK2CQR/P")?;
    *shared = AcceptedRecord::new(
        shared.key(),
        RecordState::Live(RecordValue::Qso(Box::new(values.try_into()?))),
        shared.stamp.clone(),
    )?;
    sync_record(&mut newer);
    draft.observe(&newer);
    assert!(draft.is_stale());
    assert!(draft.build(&DefinitionSet::default()).is_err());
    draft.refresh(newer)?;
    assert!(!draft.is_stale());
    assert!(draft.build(&DefinitionSet::default()).is_err());
    assert!(
        matches!(draft.input(FieldId::Qso(QsoField::Comment))?, FieldInput::Text { value } if value == "My combined comment")
    );
    draft.confirm(FieldId::Qso(QsoField::Comment));
    draft.select(
        FieldId::Qso(QsoField::Station {
            side: clacks_model::reference::StationSide::Contacted,
            field: StationField::Callsign,
        }),
        ComparisonSource::Shared,
    )?;
    assert_eq!(
        qso(&draft.build(&DefinitionSet::default()).unwrap())
            .comment()
            .as_deref(),
        Some("My combined comment")
    );
    Ok(())
}

fn sync_record(basis: &mut ComparisonBasis) {
    let mut fields = basis.record.clone().into_fields();
    fields.latest = basis.review.shared.clone();
    basis.record = fields.try_into().unwrap();
}

#[test]
fn saved_source_fields_require_explicit_choices_without_loading_operating_values()
-> anyhow::Result<()> {
    use super::fields::ProfileField;
    let station = clacks_model::contact::StationSnapshot {
        callsign: RecordedCallsign::new("W7HEX")?,
        location: None,
        references: Default::default(),
    };
    let profile: clacks_model::profile::SavedLocationProfile =
        clacks_model::profile::SavedLocationProfileFields {
            id: Default::default(),
            name: "Home".into(),
            station,
            default_equipment: None,
        }
        .try_into()?;
    let mut changed = profile.clone().into_fields();
    changed.name = "Portable".into();
    let basis = saved_basis(
        RecordValue::Profile(Box::new(profile.clone())),
        RecordValue::Profile(Box::new(changed.try_into()?)),
    );
    let mut draft = ResolutionDraft::new(basis)?;
    assert!(draft.build(&Default::default()).is_err());
    draft.select(
        FieldId::Profile(ProfileField::Name),
        ComparisonSource::Shared,
    )?;
    let RecordState::Live(RecordValue::Profile(resolved)) =
        draft.build(&Default::default()).unwrap()
    else {
        panic!("profile")
    };
    assert_eq!(resolved.name(), "Portable");
    assert_eq!(resolved.station(), profile.station());
    Ok(())
}

pub(crate) fn saved_basis(local: RecordValue, shared: RecordValue) -> ComparisonBasis {
    let source = basis();
    let key = RecordKey {
        account: source.record.key().account,
        id: local.id(),
    };
    let Observation::Accepted(old) = source.review.shared else {
        panic!("accepted")
    };
    let shared = Observation::Accepted(
        AcceptedRecord::new(key, RecordState::Live(shared), old.stamp).unwrap(),
    );
    let local = RecordState::Live(local);
    let original = OriginalBasis::Unavailable {
        key,
        reason: clacks_sync::error::UnknownReason::MissingOriginal,
    };
    let record = clacks_sync::client::ClientRecordFields {
        key,
        latest: shared.clone(),
        retained: vec![],
        pending: Some(PendingRecord {
            key,
            original: original.clone(),
            working: local.clone(),
            prepared: None,
            witness: None,
        }),
    }
    .try_into()
    .unwrap();
    ComparisonBasis {
        record,
        review: ReviewInput {
            original,
            local,
            shared,
            reason: ReviewReason::MissingOriginal,
            retained: vec![],
        },
    }
}

#[test]
fn saved_equipment_and_independent_upload_policies_resolve_as_native_fields() -> anyhow::Result<()>
{
    use super::fields::{EquipmentField, TagField};
    use clacks_model::{
        profile::EquipmentConfigurationFields,
        tag::{AutomaticUploadPolicy, LogbookTag},
    };
    let original: clacks_model::profile::EquipmentConfiguration = EquipmentConfigurationFields {
        id: Default::default(),
        name: "Rig A".into(),
        values: clacks_model::equipment::EquipmentValuesFields {
            equipment_description: Some("KX3".into()),
            ..Default::default()
        },
    }
    .try_into()?;
    let mut changed = original.clone().into_fields();
    changed.values.antenna_description = Some("Vertical".into());
    let mut draft = ResolutionDraft::new(saved_basis(
        RecordValue::Equipment(Box::new(original)),
        RecordValue::Equipment(Box::new(changed.try_into()?)),
    ))?;
    draft.select(
        FieldId::Equipment(EquipmentField::Antenna),
        ComparisonSource::Shared,
    )?;
    draft.set_custom(
        FieldId::Equipment(EquipmentField::Name),
        FieldInput::Text {
            value: "Portable rig".into(),
        },
    );
    let RecordState::Live(RecordValue::Equipment(value)) =
        draft.build(&Default::default()).unwrap()
    else {
        panic!("equipment")
    };
    assert_eq!(value.name(), "Portable rig");
    assert_eq!(
        value.values().equipment_description().as_deref(),
        Some("KX3")
    );
    assert_eq!(
        value.values().antenna_description().as_deref(),
        Some("Vertical")
    );
    let mut local = LogbookTag::new(Default::default(), "Main".into());
    local.automatic_upload.lotw = AutomaticUploadPolicy::Enable;
    let mut shared = local.clone();
    shared.automatic_upload.lotw = AutomaticUploadPolicy::Forbid;
    shared.automatic_upload.qrz = AutomaticUploadPolicy::Enable;
    let mut draft = ResolutionDraft::new(saved_basis(
        RecordValue::Tag(Box::new(local)),
        RecordValue::Tag(Box::new(shared)),
    ))?;
    draft.select(FieldId::Tag(TagField::Lotw), ComparisonSource::Local)?;
    draft.select(FieldId::Tag(TagField::Qrz), ComparisonSource::Shared)?;
    let RecordState::Live(RecordValue::Tag(value)) = draft.build(&Default::default()).unwrap()
    else {
        panic!("tag")
    };
    assert_eq!(value.automatic_upload.lotw, AutomaticUploadPolicy::Enable);
    assert_eq!(value.automatic_upload.qrz, AutomaticUploadPolicy::Enable);
    Ok(())
}

#[test]
fn deletion_and_live_versions_require_an_explicit_state_choice() -> anyhow::Result<()> {
    for local_deleted in [true, false] {
        let mut basis = basis();
        if local_deleted {
            basis.review.local = RecordState::Deleted;
            let mut fields = basis.record.clone().into_fields();
            fields.pending.as_mut().unwrap().working = RecordState::Deleted;
            basis.record = fields.try_into()?;
        } else {
            let Observation::Accepted(shared) = &basis.review.shared else {
                panic!("shared")
            };
            basis.review.shared = Observation::Accepted(AcceptedRecord::new(
                shared.key(),
                RecordState::Deleted,
                shared.stamp.clone(),
            )?);
            sync_record(&mut basis);
        }
        let mut draft = ResolutionDraft::new(basis)?;
        assert!(draft.build(&Default::default()).is_err());
        draft.select_all(if local_deleted {
            ComparisonSource::Local
        } else {
            ComparisonSource::Shared
        })?;
        assert_eq!(
            draft.build(&Default::default()).unwrap(),
            RecordState::Deleted
        );
        draft.select_all(if local_deleted {
            ComparisonSource::Shared
        } else {
            ComparisonSource::Local
        })?;
        assert!(matches!(
            draft.build(&Default::default()).unwrap(),
            RecordState::Live(_)
        ));
    }
    Ok(())
}

#[test]
fn retained_live_values_remain_selectable_when_both_current_versions_are_deleted()
-> anyhow::Result<()> {
    let mut basis = basis();
    let Observation::Accepted(shared) = &basis.review.shared else {
        panic!("accepted")
    };
    let retained = AcceptedRecord::new(
        shared.key(),
        shared.state().clone(),
        VersionStamp::new(
            HistoryPosition {
                timeline: shared.stamp.accepted_at().timeline,
                revision: 3,
            },
            shared.stamp.baseline(),
            RecordVector::from_entries([(
                ClockActor::Server(shared.stamp.accepted_at().timeline),
                3,
            )])?,
        )?,
    )?;
    let source = ComparisonSource::Retained {
        stamp: retained.stamp.clone(),
    };
    basis.review.local = RecordState::Deleted;
    basis.review.shared = Observation::Accepted(AcceptedRecord::new(
        shared.key(),
        RecordState::Deleted,
        shared.stamp.clone(),
    )?);
    basis.review.retained = vec![retained.clone()];
    basis.review.reason =
        ReviewReason::OrderingUnknown(clacks_sync::error::UnknownReason::CollectedContext);
    let mut fields = basis.record.clone().into_fields();
    fields.latest = basis.review.shared.clone();
    fields.pending.as_mut().unwrap().working = RecordState::Deleted;
    fields.retained = vec![retained.clone()];
    basis.record = fields.try_into()?;
    let mut draft = ResolutionDraft::new(basis)?;
    assert!(draft.build(&Default::default()).is_err());
    draft.select_all(source)?;
    assert_eq!(&draft.build(&Default::default()).unwrap(), retained.state());
    Ok(())
}

#[test]
fn refreshed_live_changes_require_renewing_a_deletion_choice() -> anyhow::Result<()> {
    let mut basis = basis();
    basis.review.local = RecordState::Deleted;
    let mut fields = basis.record.clone().into_fields();
    fields.pending.as_mut().unwrap().working = RecordState::Deleted;
    basis.record = fields.try_into()?;
    let mut draft = ResolutionDraft::new(basis.clone())?;
    draft.select_all(ComparisonSource::Local)?;
    assert_eq!(
        draft.build(&Default::default()).unwrap(),
        RecordState::Deleted
    );

    let Observation::Accepted(shared) = &mut basis.review.shared else {
        panic!("shared")
    };
    let mut values = qso(shared.state()).clone().into_fields();
    values.comment = Some("New on-air correction".into());
    *shared = AcceptedRecord::new(
        shared.key(),
        RecordState::Live(RecordValue::Qso(Box::new(values.try_into()?))),
        shared.stamp.clone(),
    )?;
    sync_record(&mut basis);
    draft.observe(&basis);
    draft.refresh(basis)?;
    assert_eq!(draft.deletion_choice(), None);
    assert!(!draft.fields().is_empty());
    assert!(draft.build(&Default::default()).is_err());
    assert!(matches!(
        draft.source_input(FieldId::Qso(QsoField::Comment), &ComparisonSource::Shared)?,
        FieldInput::Text { value } if value == "New on-air correction"
    ));
    draft.select_all(ComparisonSource::Local)?;
    assert_eq!(
        draft.build(&Default::default()).unwrap(),
        RecordState::Deleted
    );
    Ok(())
}
