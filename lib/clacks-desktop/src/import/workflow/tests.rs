//! Import workflows covering assignment, saved effects, and partial application.

use super::{Action, PrepareRequest, Report, ReviewSession, RowReadiness, operation::run};
use crate::import::{adi::prepare_source, mapping::Choice};
use crate::synchronization::ExchangeGate;
use clacks_client_store::{read, save, store::ClientStore};
use clacks_model::{
    contact::Qso,
    defaults::account_definitions,
    ids::{AccountId, QsoId, TagId},
    service::evidence::ServicePayload,
    tag::LogbookTag,
};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use tokio::sync::{oneshot, watch};
use tokio_util::sync::CancellationToken;
use tracing::instrument::WithSubscriber;

fn row(worked: &str, start: &str, optional: &str) -> String {
    format!(
        "<CALL:{}>{worked}<STATION_CALLSIGN:5>W7ABC<QSO_DATE:8>20261119<TIME_ON:4>{start}<BAND:3>20M<MODE:3>FT8{optional}<EOR>\n",
        worked.len()
    )
}

type StartedImport = (
    oneshot::Receiver<ReviewSession>,
    oneshot::Receiver<Result<Report, String>>,
    watch::Receiver<usize>,
    tokio::task::JoinHandle<()>,
);

fn begin(
    store: &ClientStore,
    gate: &ExchangeGate,
    tag: TagId,
    path: PathBuf,
    stop: CancellationToken,
) -> StartedImport {
    let (review, prepared) = oneshot::channel();
    let (completion, applied) = oneshot::channel();
    let (progress, row_progress) = watch::channel(0);
    let task = tokio::spawn(
        run(
            store.clone(),
            gate.clone(),
            PrepareRequest {
                path,
                tag,
                stop,
                review,
                completion,
                progress,
            },
            watch::channel(()).0,
            Arc::new(OnceLock::new()),
        )
        .with_current_subscriber(),
    );
    (prepared, applied, row_progress, task)
}

#[test]
fn adi_import_applies_reviewed_one_to_one_matches_and_only_the_readable_prefix()
-> anyhow::Result<()> {
    crate::telemetry::tests::child_case("adi-import")
}

pub(crate) fn run_isolated() -> anyhow::Result<()> {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};
    use serde_json::Value;

    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_span_list(true)
        .with_writer(std::sync::Mutex::new(output.reopen()?))
        .finish();
    let recorder = DebuggingRecorder::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    tracing::subscriber::with_default(subscriber, || {
        metrics::with_local_recorder(&recorder, || runtime.block_on(apply_reviewed_matches()))
    })?;

    let logged = std::fs::read_to_string(output.path())?;
    for private in ["historical.adi", "Boise", "Reno", "K1XYZ", "W7ABC"] {
        assert!(
            !logged.contains(private),
            "import telemetry exposed {private}"
        );
    }
    let events = logged
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let settled = events
        .iter()
        .find(|event| {
            event["fields"]["event"] == "desktop.import.settled"
                && event["fields"]["outcome"] == "completed_with_failures"
        })
        .expect("the settled event should report completed_with_failures because the import applied valid rows and rejected one");
    assert_eq!(settled["fields"]["created"], 1);
    assert_eq!(settled["fields"]["unchanged"], 2);
    assert_eq!(settled["fields"]["failed"], 1);
    assert_eq!(settled["fields"]["source_truncated"], true);
    let import_id = |event: &Value| {
        event["spans"].as_array()?.iter().find_map(|span| {
            (span["name"] == "desktop.import")
                .then(|| span["run_id"].as_str().map(str::to_owned))
                .flatten()
        })
    };
    let settled_run = import_id(settled).expect("the settled event should carry the import run ID because it is emitted inside that import's span");
    for event_name in [
        "desktop.import.preparing",
        "desktop.import.ready_for_review",
        "desktop.import.applying",
        "desktop.import.row_failures",
    ] {
        assert!(events.iter().any(|event| {
            event["fields"]["event"] == event_name
                && import_id(event).as_deref() == Some(settled_run.as_str())
        }));
    }

    let metrics = recorder.snapshotter().snapshot().into_vec();
    let (key, _, _, value) = metrics
        .iter()
        .find(|(key, _, _, _)| key.key().name() == "clacks_desktop_import_row_failures_total")
        .expect("the row failure counter should exist because the reviewed end preceded the saved start");
    let labels: Vec<_> = key
        .key()
        .labels()
        .map(|label| (label.key(), label.value()))
        .collect();
    assert_eq!(labels, [("reason", "planning")]);
    assert_eq!(*value, DebugValue::Counter(1));
    Ok(())
}

async fn apply_reviewed_matches() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let account = AccountId::new();
    let store = ClientStore::create_with_definitions(
        &directory.path().join("workspace.sqlite"),
        account,
        &account_definitions()?,
        Default::default(),
    )
    .await?;
    let tag = TagId::new();
    save::save(
        &store,
        RecordKey {
            account,
            id: RecordId::Tag(tag),
        },
        None,
        RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
            tag,
            "History".into(),
        )))),
    )
    .await?;

    let mut snapshot = read::snapshot(&store).await?;
    let context = snapshot.import_context().await?;
    snapshot.close().await?;
    let mut ids = vec![];
    for input in [
        row("K1ABC", "1200", ""),
        row("K1ABC", "1240", ""),
        row("K1DEF", "1200", ""),
        row("K1ABC/P", "1200", ""),
        row("K1ABC", "1200", "").replace("<MODE:3>FT8", "<MODE:2>CW"),
    ] {
        let seed = prepare_source(input.as_bytes(), context.definitions());
        let prepared = seed
            .rows
            .first()
            .ok_or_else(|| anyhow::anyhow!("Known complete seed row"))?;
        let id = QsoId::new();
        let qso = Qso::try_from(prepared.resolve(&BTreeMap::new()).create(id, None)?)?;
        save::contacts::save(&store, None, qso, None).await?;
        ids.push(id);
    }
    let [x, y, z, portable, different_mode] = ids.as_slice() else {
        anyhow::bail!("The fixture saves its candidates and both incompatible contacts")
    };

    let path = directory.path().join("historical.adi");
    let contents = row("K1ABC", "1215", "<COMMENT:4>Reno<COMMENT:5>Boise")
        + &row("K1ABC", "1140", "")
        + &row(
            "K1XYZ",
            "1300",
            "<COMMENT:4>Reno<COMMENT:5>Boise<QTH:4>Reno<MY_CITY:5>Salem<LOTW_QSL_SENT:1>Y<LOTW_QSLSDATE:8>20261120",
        )
        + &row("K1DEF", "1140", "<TIME_ON:4>1500<TIME_OFF:4>1145")
        + "<CALL:20>cut";
    tokio::fs::write(&path, contents).await?;

    let gate = ExchangeGate::default();
    let stop = CancellationToken::new();
    let (prepared, outcome, _, task) = begin(&store, &gate, tag, path.clone(), stop.clone());
    let ReviewSession {
        prepared: first,
        apply,
    } = prepared.await?;
    assert_eq!(first.rows.len(), 4);
    assert!(first.terminal.is_some());
    assert_eq!(first.action(0), None);
    assert_eq!(first.action(1), None);
    drop(apply);
    stop.cancel();
    assert!(outcome.await?.map_err(anyhow::Error::msg)?.cancelled);
    task.await?;
    let mut snapshot = read::snapshot(&store).await?;
    assert_eq!(
        snapshot
            .contact_page(&Default::default())
            .await?
            .matching_total,
        5
    );
    snapshot.close().await?;

    let (prepared, outcome, _, task) =
        begin(&store, &gate, tag, path.clone(), CancellationToken::new());
    let ReviewSession {
        mut prepared,
        apply,
    } = prepared.await?;
    assert_eq!(prepared.rows[0].candidates.len(), 2);
    assert_eq!(prepared.rows[1].candidates.len(), 1);
    assert!(prepared.saved(*portable).is_none());
    assert!(prepared.saved(*different_mode).is_none());
    prepared.resolve(0, 0, Choice::Use(0));
    prepared.select(0, Action::Update(*x));
    prepared.select(1, Action::Update(*x));
    assert_eq!(prepared.action(1), None);
    prepared.select(0, Action::Update(*y));
    prepared.select(1, Action::Update(*x));
    for (row, target) in [(0, *y), (1, *x)] {
        let (_, differences) = prepared.review_comparison(row, target)?;
        assert!(!differences.is_empty());
        prepared.rows[row]
            .fields
            .extend(differences.iter().map(|delta| (delta.field, false)));
    }
    prepared.resolve(0, 0, Choice::Use(1));
    assert_eq!(prepared.action(0), Some(Action::Update(*y)));
    let (_, differences) = prepared.review_comparison(0, *y)?;
    assert!(
        differences
            .iter()
            .any(|delta| prepared.rows[0].fields.get(&delta.field) == Some(&false))
    );
    for delta in differences {
        prepared.rows[0].fields.insert(delta.field, false);
    }
    use crate::review::fields::{FieldId, QsoField};
    prepared.resolve(3, 0, Choice::Use(0));
    prepared.select(3, Action::Update(*z));
    let (_, differences) = prepared.review_comparison(3, *z)?;
    prepared.rows[3]
        .fields
        .extend(differences.into_iter().map(|delta| (delta.field, false)));
    assert!(matches!(prepared.readiness(3)?, RowReadiness::Ready { .. }));
    prepared.rows[3]
        .fields
        .insert(FieldId::Qso(QsoField::End), true);
    let invalid = prepared
        .readiness(3)
        .err()
        .expect("readiness should return an error because the selected end precedes the retained saved start");
    assert!(matches!(
        invalid.downcast_ref::<clacks_model::contact::RecordError>(),
        Some(clacks_model::contact::RecordError::EndBeforeStart { .. })
    ));

    assert_eq!(prepared.rows[2].source.disputes.len(), 1);
    prepared.resolve(2, 0, Choice::Use(1));
    assert!(apply.send(prepared).is_ok());

    let report = outcome.await?.map_err(anyhow::Error::msg)?;
    assert_eq!(report.created, 1);
    assert_eq!(report.changed, 0);
    assert_eq!(report.unchanged, 2);
    assert_eq!(report.memberships, 3);
    assert_eq!(report.assertions, 1);
    assert!(report.terminal.is_some());
    assert_eq!(report.failures, vec![(4, invalid.to_string())]);
    assert!(report.skipped.is_empty());
    task.await?;

    let mut snapshot = read::snapshot(&store).await?;
    let records = snapshot.contact_page(&Default::default()).await?.rows;
    assert_eq!(records.len(), 6);
    let untouched = records
        .iter()
        .map(|record| &record.working.qso)
        .find(|qso| qso.id() == *z)
        .expect("the original QSO should still be present because its proposed end failed validation before any write");
    assert!(untouched.end().is_none());
    assert!(!untouched.tags().contains(&tag));
    let imported = records
        .iter()
        .map(|record| &record.working.qso)
        .find(|qso| qso.contacted_station().callsign.as_str() == "K1XYZ")
        .ok_or_else(|| anyhow::anyhow!("The reviewed third source row creates K1XYZ"))?;
    assert_eq!(imported.comment().as_deref(), Some("Boise"));
    assert!(imported.contacted_station().location.is_none());
    assert!(imported.own_station().location.is_none());
    assert_eq!(report.unsupported.get("QTH"), Some(&1));
    assert_eq!(report.unsupported.get("MY_CITY"), Some(&1));
    snapshot.close().await?;
    let (prepared, outcome, _, task) = begin(&store, &gate, tag, path, CancellationToken::new());
    let ReviewSession {
        mut prepared,
        apply,
    } = prepared.await?;
    prepared.resolve(2, 0, Choice::Use(1));
    assert!(matches!(prepared.action(2), Some(Action::Update(_))));
    assert!(apply.send(prepared).is_ok());
    let report = outcome.await?.map_err(anyhow::Error::msg)?;
    assert_eq!(report.unchanged, 1);
    assert_eq!(report.unprocessed, 3);
    assert_eq!(report.assertions, 0);
    assert_eq!(report.memberships, 0);
    assert!(report.failures.is_empty());
    task.await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn cancelling_adi_application_preserves_complete_contacts_and_assertions()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let account = AccountId::new();
    let store = ClientStore::create_with_definitions(
        &directory.path().join("workspace.sqlite"),
        account,
        &account_definitions()?,
        Default::default(),
    )
    .await?;
    let tag = TagId::new();
    save::save(
        &store,
        RecordKey {
            account,
            id: RecordId::Tag(tag),
        },
        None,
        RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
            tag,
            "History".into(),
        )))),
    )
    .await?;
    let path = directory.path().join("cancel.adi");
    tokio::fs::write(&path, row("K1ABC", "1200", "<LOTW_QSL_SENT:1>Y").repeat(8)).await?;
    let gate = ExchangeGate::default();
    let stop = CancellationToken::new();
    let (prepared, outcome, mut progress, task) = begin(&store, &gate, tag, path, stop.clone());
    let ReviewSession { prepared, apply } = prepared.await?;
    assert!(apply.send(prepared).is_ok());
    progress.changed().await?;
    stop.cancel();
    let report = outcome.await?.map_err(anyhow::Error::msg)?;
    task.await?;
    assert!(report.cancelled);
    assert!((1..8).contains(&report.created));
    assert_eq!(report.unprocessed + report.created, 8);
    assert_eq!(report.assertions, report.created);
    let mut snapshot = read::snapshot(&store).await?;
    let contacts = snapshot.contact_page(&Default::default()).await?.rows;
    assert_eq!(contacts.len(), report.created);
    snapshot.close().await?;

    let contact_ids = contacts
        .iter()
        .map(|record| RecordId::Qso(record.working.qso.id()))
        .collect::<BTreeSet<_>>();
    let pending = clacks_client_store::synchronization::state(&store)
        .await?
        .pending;
    let assertion_contacts = pending
        .iter()
        .filter_map(|entry| {
            let Some(RecordState::Live(RecordValue::Service(service))) =
                entry.record.working_state()
            else {
                return None;
            };
            match &service.payload {
                ServicePayload::Imported(assertion) => Some(RecordId::Qso(assertion.qso)),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(assertion_contacts.len(), contacts.len());
    assert_eq!(
        assertion_contacts.into_iter().collect::<BTreeSet<_>>(),
        contact_ids
    );
    store.close().await?;
    Ok(())
}
