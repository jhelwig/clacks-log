use clacks_model::{
    ids::{AcquisitionId, DestinationId, ObservationId},
    scalar::{UtcSecond, UtcTimestamp},
    service::{
        destination::{ExternalRecordKey, RemoteRecordId},
        evidence::observation::*,
    },
};

fn observation() -> RemoteObservation {
    let d = DestinationId::new();
    RemoteObservation {
        id: ObservationId::new(),
        destination: d,
        identity: RemoteIdentity::Complete(ExternalRecordKey {
            destination: d,
            record: RemoteRecordId::Qrz("7".parse().unwrap()),
        }),
        facts: RemoteFacts::default(),
        ordering: ProviderOrdering::default(),
        acquisition: AcquisitionProvenance {
            id: AcquisitionId::new(),
            query: QueryIdentity {
                destination: d,
                stream: QueryStream::Qrz,
                selected_fields: Default::default(),
                own_call: None,
                since: None,
            },
            observed_at: UtcTimestamp::new(1, 0).unwrap(),
        },
    }
}

#[test]
fn missing_fields_do_not_clear_and_negative_is_a_value() -> anyhow::Result<()> {
    let mut o = observation();
    o.facts.contact.confirmation = Reported::Value(ConfirmationStatus::Confirmed);
    let current = EstablishedObservation::from(&o);
    let mut incoming = o.clone();
    incoming.facts.contact.confirmation = Reported::NotReported;
    assert!(matches!(
        compare(&current, &incoming)?,
        ObservationComparison::Unchanged
    ));
    incoming.facts.contact.confirmation = Reported::Value(ConfirmationStatus::Unconfirmed);
    assert!(matches!(
        compare(&current, &incoming)?,
        ObservationComparison::Changed(_)
    ));
    Ok(())
}

#[test]
fn content_transitions_and_ordering_are_separate() -> anyhow::Result<()> {
    let a = observation();
    let mut b = a.clone();
    b.facts.contact.mode = Reported::Value(ModeReport {
        concrete: "FT8".into(),
        parent: Some("MFSK".into()),
        group: Some("DATA".into()),
    });
    assert!(matches!(
        compare(&EstablishedObservation::from(&a), &b)?,
        ObservationComparison::Additional(_)
    ));
    let mut fresh = a.clone();
    fresh.facts.contact.mode = Reported::Value(ModeReport {
        concrete: "FT4".into(),
        parent: Some("MFSK".into()),
        group: Some("DATA".into()),
    });
    assert!(matches!(
        compare(&EstablishedObservation::from(&b), &fresh)?,
        ObservationComparison::Changed(_)
    ));
    let mut newer = b.clone();
    newer.ordering.qso = Some(UtcSecond::from_unix_seconds(99));
    assert!(matches!(
        compare(&EstablishedObservation::from(&b), &newer)?,
        ObservationComparison::OrderingOnly
    ));
    Ok(())
}
