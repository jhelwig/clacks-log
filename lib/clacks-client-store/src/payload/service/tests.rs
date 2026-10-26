//! Persistence boundaries consumed by service selection and malformed-input admission.

use super::*;
use crate::test_support as support;
use crate::{
    save,
    store::{ClientStore, StoreSettings},
    transfer,
};
use clacks_model::{
    scalar::{Hertz, RecordedCallsign, UtcSecond, UtcTimestamp},
    service::{
        destination::{
            Destination, ExternalRecordKey, LotwRecordKey, RemoteCollection, RemoteRecordId,
        },
        evidence::{ServicePayload, observation::*},
    },
};
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    lineage::Coverage,
    record::{Observation, RecordState, RecordValue},
    service::{
        ServiceView,
        combination::{ObservationSelectionInput, select_observations},
    },
    transfer::{RecordRequirement, WindowManifest, WindowMode},
};
use support::id;

fn destination() -> Destination {
    Destination {
        id: id(100),
        collection: RemoteCollection::Qrz {
            book_id: Some("18446744073709551616000".parse().unwrap()),
        },
        description: "Historical namespace".into(),
    }
}

fn report(n: u128, at: i64) -> anyhow::Result<RemoteObservation> {
    Ok(RemoteObservation {
        id: id(n),
        destination: id(100),
        identity: RemoteIdentity::Complete(ExternalRecordKey {
            destination: id(100),
            record: RemoteRecordId::Qrz("18446744073709551616001".parse()?),
        }),
        facts: RemoteFacts {
            contact: ContactReport {
                frequency: Reported::Value(Hertz::new(14_074_000)?),
                ..Default::default()
            },
            ..Default::default()
        },
        ordering: ProviderOrdering {
            qso: Some(UtcSecond::from_unix_seconds(at)),
            confirmation: None,
        },
        acquisition: AcquisitionProvenance {
            id: id(160),
            query: QueryIdentity {
                destination: id(100),
                stream: QueryStream::Qrz,
                selected_fields: Default::default(),
                own_call: None,
                since: None,
            },
            observed_at: UtcTimestamp::new(1000, 123456789)?,
        },
    })
}

#[tokio::test]
async fn restored_observations_select_provider_fields_using_retained_coverage_and_order()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let mut earlier = report(170, 10)?;
    earlier.facts.own_station.name = Reported::Value("Earlier station".into());
    earlier.facts.own_station.dxcc = Reported::Value("291".into());
    let mut later = report(171, 20)?;
    later.facts.contact.frequency = Reported::NotReported;
    later.facts.own_station.name = Reported::Clear;
    let external = ExternalRecordKey {
        destination: id(100),
        record: RemoteRecordId::Qrz("18446744073709551616001".parse()?),
    };
    let rows: Vec<_> = [earlier, later]
        .into_iter()
        .enumerate()
        .map(|(n, report)| {
            support::accepted(
                n as u64 + 1,
                RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                    payload: ServicePayload::Observation(Box::new(report)),
                    support: ServiceSupport {
                        destination: Some(destination()),
                        ..Default::default()
                    },
                }))),
            )
        })
        .collect();
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: baseline.id,
        coverage: Coverage::from(rows.last().unwrap().stamp.accepted_at),
    };
    let manifest = WindowManifest::new(
        id(1),
        source.clone(),
        WindowMode::Full,
        rows.iter()
            .map(|row| RecordRequirement {
                key: row.key,
                stamp: row.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        vec![baseline.clone()],
        vec![],
    )?;
    transfer::remember(&store, &manifest).await?;
    transfer::install(&store, &source, &WindowMode::Full, &rows).await?;
    store.close().await?;

    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let mut restored = Vec::new();
    for row in rows {
        let Observation::Accepted(value) = save::load(&store, row.key).await?.unwrap().latest
        else {
            panic!("installed service observation")
        };
        restored.push(value);
    }
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };
    let view = ServiceView {
        account: id(1),
        records: &restored,
        bindings: &[],
        evidence: &evidence,
    };
    let selected = select_observations(&ObservationSelectionInput {
        external: &external,
        view: &view,
    })?;
    assert!(selected.uncertain.is_empty());
    assert_eq!(selected.supporting, vec![id(170), id(171)]);
    assert_eq!(
        selected.current.facts.contact.frequency,
        Reported::Value(Hertz::new(14_074_000)?)
    );
    assert_eq!(selected.current.facts.own_station.name, Reported::Clear);
    assert_eq!(
        selected.current.facts.own_station.dxcc,
        Reported::Value("291".into())
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn service_schema_qualifies_exact_external_keys_and_rejects_malformed_inputs()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let mut tx = store.write().await?;
    let exact = ExternalRecordKey {
        destination: id(100),
        record: RemoteRecordId::Qrz("18446744073709551616001".parse()?),
    };
    let first = identity::external(&mut tx, id(1), &exact).await?;
    assert_eq!(identity::external(&mut tx, id(1), &exact).await?, first);
    let mut other = exact.clone();
    other.destination = id(101);
    assert_ne!(identity::external(&mut tx, id(1), &other).await?, first);
    let lotw_key = LotwRecordKey {
        own_call: RecordedCallsign::new("W7HEX/P")?,
        worked_call: RecordedCallsign::new("K1ABC")?,
        start: UtcSecond::from_unix_seconds(600),
        band: "20M".into(),
        receive_band: "2M".into(),
        mode: "DATA".into(),
    };
    let exact = ExternalRecordKey {
        destination: id(102),
        record: RemoteRecordId::Lotw(lotw_key.clone()),
    };
    let first = identity::external(&mut tx, id(1), &exact).await?;
    let near = ExternalRecordKey {
        destination: id(102),
        record: RemoteRecordId::Lotw(LotwRecordKey {
            start: UtcSecond::from_unix_seconds(601),
            ..lotw_key
        }),
    };
    assert_ne!(identity::external(&mut tx, id(1), &near).await?, first);
    let mut roots = Vec::new();
    for payload in [
        ServicePayload::Destination(destination()),
        ServicePayload::Observation(Box::new(report(170, 10)?)),
    ] {
        let value = RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport::default(),
        }));
        let key = RecordKey {
            account: id(1),
            id: value.id(),
        };
        let root = crate::payload::insert(&mut tx, key, &RecordState::Live(value)).await?;
        roots.push((key, root));
    }
    let mut roots = roots.into_iter();
    let (destination_key, destination_root) = roots.next().unwrap();
    let (_, observation_root) = roots.next().unwrap();
    assert!(
        crate::payload::load(
            &mut tx,
            RecordKey {
                account: id(2),
                ..destination_key
            },
            destination_root
        )
        .await
        .is_err()
    );
    assert!(
        sqlx::query_file!("tests/sql/invalid_report_frequency.sql", observation_root)
            .execute(&mut *tx)
            .await
            .is_err()
    );
    sqlx::query_file!("tests/sql/orphan_service_map.sql", destination_root)
        .execute(&mut *tx)
        .await?;
    assert!(
        crate::payload::load(&mut tx, destination_key, destination_root)
            .await
            .is_err()
    );
    tx.rollback().await?;
    store.close().await?;
    Ok(())
}
