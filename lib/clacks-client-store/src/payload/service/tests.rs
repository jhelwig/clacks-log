//! Stored provider coverage and ordering consumed by observation selection.

use super::*;
use crate::test_support as support;
use crate::{
    save,
    store::{ClientStore, StoreSettings},
    transfer,
};
use clacks_model::{
    scalar::{Hertz, UtcSecond, UtcTimestamp},
    service::{
        destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
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
                selected_fields: ["CALL".into(), "FREQ".into()].into(),
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
        coverage: Coverage::from(rows.last().unwrap().stamp.accepted_at()),
    };
    let manifest = WindowManifest::new(
        id(1),
        source.clone(),
        WindowMode::Full,
        rows.iter()
            .map(|row| RecordRequirement {
                key: row.key(),
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
        let Observation::Accepted(value) = save::load(&store, row.key())
            .await?
            .unwrap()
            .into_fields()
            .latest
        else {
            panic!("installed service observation")
        };
        if let RecordState::Live(RecordValue::Service(record)) = value.state() {
            let ServicePayload::Observation(observation) = &record.payload else {
                panic!("observation")
            };
            assert_eq!(
                observation.acquisition.query.selected_fields,
                ["CALL".into(), "FREQ".into()].into()
            );
        }
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
async fn exact_provider_keys_remain_distinct_across_reopen() -> anyhow::Result<()> {
    use clacks_model::{scalar::RecordedCallsign, service::destination::LotwRecordKey};
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let base = LotwRecordKey {
        own_call: RecordedCallsign::new("W7HEX/P")?,
        worked_call: RecordedCallsign::new("K1ABC")?,
        start: UtcSecond::from_unix_seconds(1000),
        band: "20M".into(),
        receive_band: "20M".into(),
        mode: "FT8".into(),
    };
    let mut receive = base.clone();
    receive.receive_band = "40M".into();
    let mut mode = base.clone();
    mode.mode = "DATA".into();
    let keys: Vec<_> = [
        RemoteRecordId::Lotw(base),
        RemoteRecordId::Lotw(receive),
        RemoteRecordId::Lotw(mode),
        RemoteRecordId::Qrz("18446744073709551616001".parse()?),
    ]
    .into_iter()
    .map(|record| ExternalRecordKey {
        destination: id(100),
        record,
    })
    .collect();
    let mut tx = store.write().await?;
    let mut retained = Vec::new();
    for key in &keys {
        let stored = identity::external(&mut tx, id(1), key).await?;
        assert_eq!(identity::external(&mut tx, id(1), key).await?, stored);
        retained.push(stored);
    }
    assert_eq!(
        retained
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        keys.len()
    );
    tx.commit().await?;
    store.close().await?;

    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let mut tx = store.write().await?;
    for (stored, key) in retained.into_iter().zip(keys) {
        assert_eq!(identity::load_external(&mut tx, id(1), stored).await?, key);
    }
    tx.commit().await?;
    store.close().await
}
