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
        account: Default::default(),
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
