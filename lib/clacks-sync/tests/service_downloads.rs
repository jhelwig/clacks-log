//! Provider observation selection from fixed accepted evidence, without simulated workers.

use crate::support;
use clacks_model::{
    scalar::{UtcSecond, UtcTimestamp},
    service::{
        destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
        evidence::{ServicePayload, observation::*},
    },
};
use clacks_sync::{
    account::{AccountMetadata, EvidenceView},
    record::{AcceptedRecord, RecordId, RecordState, RecordValue},
    service::{
        ServiceRecord, ServiceSupport, ServiceView,
        combination::{ObservationSelectionInput, select_observations},
    },
};

fn external() -> ExternalRecordKey {
    ExternalRecordKey {
        destination: support::id(42),
        record: RemoteRecordId::Qrz("7".parse().unwrap()),
    }
}

fn observation(
    n: u128,
    name: &str,
    qso: Option<i64>,
    confirmed: Option<i64>,
    status: ConfirmationStatus,
    acquisition: u64,
) -> anyhow::Result<AcceptedRecord> {
    let observation = RemoteObservation {
        id: support::id(n),
        destination: support::id(42),
        identity: RemoteIdentity::Complete(external()),
        facts: RemoteFacts {
            own_station: StationReport {
                name: Reported::Value(name.into()),
                ..Default::default()
            },
            contact: ContactReport {
                confirmation: Reported::Value(status),
                ..Default::default()
            },
            ..Default::default()
        },
        ordering: ProviderOrdering {
            qso: qso.map(UtcSecond::from_unix_seconds),
            confirmation: confirmed.map(UtcSecond::from_unix_seconds),
        },
        acquisition: AcquisitionProvenance {
            id: support::id(n + 100),
            query: QueryIdentity {
                destination: support::id(42),
                stream: QueryStream::Qrz,
                selected_fields: Default::default(),
                own_call: None,
                since: None,
            },
            observed_at: UtcTimestamp::new(0, 0)?,
        },
    };
    let mut row = support::accepted(n as u64, "fixture");
    support::edit_accepted(&mut row, |row| {
        row.key.id = RecordId::Service(
            clacks_model::service::evidence::ServiceRecordId::Observation(observation.id),
        );
        row.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Observation(Box::new(observation)),
            support: ServiceSupport {
                acquisition: Some(support::accepted(acquisition, "start").stamp),
                destination: Some(Destination {
                    id: support::id(42),
                    collection: RemoteCollection::Qrz {
                        book_id: Some("7".parse().unwrap()),
                    },
                    description: "QRZ logbook".into(),
                }),
                ..Default::default()
            },
        })));
    });
    Ok(row)
}

#[test]
fn qso_and_confirmation_timestamps_select_fields_independently() -> anyhow::Result<()> {
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };
    let rows = [
        observation(
            101,
            "A",
            Some(100),
            Some(300),
            ConfirmationStatus::Confirmed,
            10,
        )?,
        observation(
            102,
            "B",
            Some(200),
            Some(200),
            ConfirmationStatus::Unconfirmed,
            20,
        )?,
    ];
    let view = ServiceView {
        account: support::id(1),
        records: &rows,
        bindings: &[],
        evidence: &evidence,
    };
    let selected = select_observations(&ObservationSelectionInput {
        external: &external(),
        view: &view,
    })?;
    assert_eq!(
        selected.current.facts.own_station.name,
        Reported::Value("B".into())
    );
    assert_eq!(
        selected.current.facts.contact.confirmation,
        Reported::Value(ConfirmationStatus::Confirmed)
    );
    assert_eq!(selected.confirmation_supporting, vec![support::id(101)]);
    assert!(selected.uncertain.is_empty());
    Ok(())
}

#[test]
fn conflicting_mixed_order_stays_unknown_until_dominating_evidence_arrives() -> anyhow::Result<()> {
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };
    let mut rows = vec![
        observation(
            101,
            "CANARY-A",
            None,
            Some(3),
            ConfirmationStatus::Confirmed,
            10,
        )?,
        observation(
            102,
            "CANARY-B",
            None,
            None,
            ConfirmationStatus::Unconfirmed,
            20,
        )?,
        observation(
            103,
            "CANARY-C",
            None,
            Some(1),
            ConfirmationStatus::Confirmed,
            30,
        )?,
    ];
    let view = ServiceView {
        account: support::id(1),
        records: &rows,
        bindings: &[],
        evidence: &evidence,
    };
    let captured = support::telemetry::capture(|| {
        select_observations(&ObservationSelectionInput {
            external: &external(),
            view: &view,
        })
    });
    assert!(!format!("{:?}", captured.observations).contains("CANARY"));
    let event = captured.event("observation.field_uncertain");
    assert_eq!(
        event.fields.get("field").map(String::as_str),
        Some("contact.confirmation")
    );
    let selected = captured.result?;
    assert_eq!(
        selected.current.facts.contact.confirmation,
        Reported::NotReported
    );
    assert!(selected.uncertain.contains(&"contact.confirmation".into()));
    rows.push(observation(
        104,
        "D",
        None,
        Some(4),
        ConfirmationStatus::Confirmed,
        40,
    )?);
    let view = ServiceView {
        account: support::id(1),
        records: &rows,
        bindings: &[],
        evidence: &evidence,
    };
    let selected = select_observations(&ObservationSelectionInput {
        external: &external(),
        view: &view,
    })?;
    assert_eq!(
        selected.current.facts.contact.confirmation,
        Reported::Value(ConfirmationStatus::Confirmed)
    );
    assert!(!selected.uncertain.contains(&"contact.confirmation".into()));
    Ok(())
}
