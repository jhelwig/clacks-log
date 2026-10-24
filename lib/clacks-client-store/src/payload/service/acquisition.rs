//! Query coverage and recoverable destination download coordination.

use super::reference;
use clacks_model::{
    ids::AccountId,
    scalar::{RecordedCallsign, UtcSecond, UtcTimestamp},
    service::evidence::{
        ServicePayload, ServiceRecordId,
        download::{AcquisitionPhase, AcquisitionState, DownloadControl, StreamProgress},
        observation::{QueryIdentity, QueryStream},
    },
};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Store exact query semantics and its unordered field-coverage set.
pub(super) async fn query(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    value: &QueryIdentity,
) -> anyhow::Result<i64> {
    crate::payload::identity(
        connection,
        RecordKey {
            account,
            id: RecordId::Service(ServiceRecordId::Destination(value.destination)),
        },
    )
    .await?;
    let account = account.to_string();
    let destination = value.destination.to_string();
    let stream = match value.stream {
        QueryStream::LotwReceipt => "receipt",
        QueryStream::LotwConfirmation => "confirmation",
        QueryStream::Qrz => "qrz",
    };
    let call = value.own_call.as_ref().map(RecordedCallsign::as_str);
    let since = value.since.map(UtcSecond::unix_seconds);
    let id = sqlx::query_file!(
        "sql/service/query_insert.sql",
        payload,
        account,
        destination,
        stream,
        call,
        since
    )
    .fetch_one(&mut *connection)
    .await?
    .query_id;
    for field in &value.selected_fields {
        sqlx::query_file!("sql/service/query_field_insert.sql", id, field)
            .execute(&mut *connection)
            .await?;
    }
    Ok(id)
}

/// Restore query coverage without treating a since filter as a receipt checkpoint.
pub(super) async fn load_query(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    query: i64,
) -> anyhow::Result<QueryIdentity> {
    let account = account.to_string();
    let row = sqlx::query_file!("sql/service/query_select.sql", query, payload, account)
        .fetch_one(&mut *connection)
        .await?;
    let fields = sqlx::query_file!("sql/service/query_field_select.sql", query)
        .fetch_all(connection)
        .await?
        .into_iter()
        .map(|row| row.field)
        .collect();
    Ok(QueryIdentity {
        destination: row.destination_id.parse()?,
        stream: match row.stream.as_str() {
            "receipt" => QueryStream::LotwReceipt,
            "confirmation" => QueryStream::LotwConfirmation,
            "qrz" => QueryStream::Qrz,
            _ => anyhow::bail!("unknown query stream"),
        },
        selected_fields: fields,
        own_call: row
            .own_call
            .map(|call| RecordedCallsign::new(&call))
            .transpose()?,
        since: row.since.map(UtcSecond::from_unix_seconds),
    })
}

/// Persist download coordination or one recoverable acquisition's actual state.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
    value: &ServicePayload,
) -> anyhow::Result<()> {
    let account = key.account.to_string();
    match value {
        ServicePayload::Download(value) => {
            reference(
                connection,
                key,
                RecordId::Service(ServiceRecordId::Destination(value.destination)),
            )
            .await?;
            if let Some(active) = value.active {
                reference(
                    connection,
                    key,
                    RecordId::Service(ServiceRecordId::Acquisition(active)),
                )
                .await?;
            }
            let destination = value.destination.to_string();
            let active = value.active.map(|id| id.to_string());
            sqlx::query_file!(
                "sql/service/download_insert.sql",
                payload,
                account,
                destination,
                active
            )
            .execute(&mut *connection)
            .await?;
            for (ordinal, progress) in value.progress.iter().enumerate() {
                let ordinal = i64::try_from(ordinal)?;
                let query = query(connection, key.account, payload, &progress.query).await?;
                let watermark = progress.watermark.map(UtcSecond::unix_seconds);
                sqlx::query_file!(
                    "sql/service/progress_insert.sql",
                    payload,
                    ordinal,
                    query,
                    watermark
                )
                .execute(&mut *connection)
                .await?;
            }
        }
        ServicePayload::Acquisition(value) => {
            let query = query(connection, key.account, payload, &value.query).await?;
            let (phase, failure) = match &value.phase {
                AcquisitionPhase::Active => ("active", None),
                AcquisitionPhase::Completed => ("completed", None),
                AcquisitionPhase::Failed(reason) => ("failed", Some(reason.as_str())),
            };
            let started = value.started_at.unix_seconds();
            let started_nanos = i64::from(value.started_at.subsecond_nanoseconds());
            let finished = value.finished_at.map(UtcTimestamp::unix_seconds);
            let finished_nanos = value
                .finished_at
                .map(|at| i64::from(at.subsecond_nanoseconds()));
            sqlx::query_file!(
                "sql/service/acquisition_insert.sql",
                payload,
                account,
                query,
                phase,
                failure,
                started,
                started_nanos,
                finished,
                finished_nanos,
                value.complete
            )
            .execute(&mut *connection)
            .await?;
            for observation in &value.retained_inputs {
                reference(
                    connection,
                    key,
                    RecordId::Service(ServiceRecordId::Observation(*observation)),
                )
                .await?;
                let observation = observation.to_string();
                sqlx::query_file!(
                    "sql/service/input_insert.sql",
                    payload,
                    account,
                    observation
                )
                .execute(&mut *connection)
                .await?;
            }
        }
        _ => anyhow::bail!("not acquisition state"),
    }
    Ok(())
}

/// Restore explicit phase, complete coverage and input identity without rerunning any request.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<ServicePayload> {
    let account = key.account.to_string();
    Ok(match key.id {
        RecordId::Service(ServiceRecordId::Download(id)) => {
            let row = sqlx::query_file!("sql/service/download_select.sql", payload, account)
                .fetch_one(&mut *connection)
                .await?;
            let mut progress = Vec::new();
            for row in sqlx::query_file!("sql/service/progress_select.sql", payload)
                .fetch_all(&mut *connection)
                .await?
            {
                anyhow::ensure!(
                    row.ordinal == i64::try_from(progress.len())?,
                    "noncontiguous stream progress"
                );
                progress.push(StreamProgress {
                    query: load_query(connection, key.account, payload, row.query_id).await?,
                    watermark: row.watermark.map(UtcSecond::from_unix_seconds),
                });
            }
            ServicePayload::Download(DownloadControl {
                id,
                destination: row.destination_id.parse()?,
                active: row.active_id.map(|id| id.parse()).transpose()?,
                progress,
            })
        }
        RecordId::Service(ServiceRecordId::Acquisition(id)) => {
            let row = sqlx::query_file!("sql/service/acquisition_select.sql", payload, account)
                .fetch_one(&mut *connection)
                .await?;
            let phase = match (row.phase.as_str(), row.failure) {
                ("active", None) => AcquisitionPhase::Active,
                ("completed", None) => AcquisitionPhase::Completed,
                ("failed", Some(reason)) => AcquisitionPhase::Failed(reason),
                _ => anyhow::bail!("invalid acquisition phase fields"),
            };
            let finished_at = match (row.finished_seconds, row.finished_nanos) {
                (None, None) => None,
                (Some(seconds), Some(nanos)) => {
                    Some(UtcTimestamp::new(seconds, nanos.try_into()?)?)
                }
                _ => anyhow::bail!("partial acquisition finish timestamp"),
            };
            anyhow::ensure!(
                matches!(row.complete, 0 | 1),
                "invalid acquisition coverage flag"
            );
            let query = load_query(connection, key.account, payload, row.query_id).await?;
            let mut inputs = std::collections::BTreeSet::new();
            for input in sqlx::query_file!("sql/service/input_select.sql", payload)
                .fetch_all(connection)
                .await?
            {
                anyhow::ensure!(
                    inputs.insert(input.observation_id.parse()?),
                    "duplicate acquisition input"
                );
            }
            ServicePayload::Acquisition(AcquisitionState {
                id,
                query,
                phase,
                started_at: UtcTimestamp::new(row.started_seconds, row.started_nanos.try_into()?)?,
                finished_at,
                complete: row.complete != 0,
                retained_inputs: inputs,
            })
        }
        _ => anyhow::bail!("not acquisition state identity"),
    })
}
