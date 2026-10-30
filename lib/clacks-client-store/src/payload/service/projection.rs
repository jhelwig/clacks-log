//! Frozen submission maps and scope-qualified attempt and result facts.

use super::{identity, reference};
use anyhow::Context;
use clacks_model::{
    ids::AccountId,
    scalar::{RecordedCallsign, UtcTimestamp},
    service::{
        destination::RemoteRecordId,
        evidence::{
            ServicePayload, ServiceRecordId,
            submission::{
                DispatchPhase, ResultScope, SubmissionAttempt, SubmissionOutcome,
                SubmissionProjection, SubmissionResult,
            },
        },
        lotw::{LotwMappingProvenance, LotwSubmissionFields, PublicSigningIdentity},
        qrz::QrzSubmission,
    },
};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Encode a retained dispatch phase without inferring success.
fn phase(value: DispatchPhase) -> &'static str {
    match value {
        DispatchPhase::Intent => "intent",
        DispatchPhase::PossibleSend => "possible",
        DispatchPhase::Dispatched => "dispatched",
    }
}

/// Decode retained dispatch evidence independently from the outcome.
fn read_phase(value: &str) -> anyhow::Result<DispatchPhase> {
    match value {
        "intent" => Ok(DispatchPhase::Intent),
        "possible" => Ok(DispatchPhase::PossibleSend),
        "dispatched" => Ok(DispatchPhase::Dispatched),
        _ => anyhow::bail!("unknown dispatch phase"),
    }
}

/// Store frozen provider values without invoking current normalization or signing.
async fn projection(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
    value: &SubmissionProjection,
) -> anyhow::Result<i64> {
    reference(
        connection,
        key,
        RecordId::Service(ServiceRecordId::Destination(value.destination())),
    )
    .await?;
    let account = key.account.to_string();
    let destination = value.destination().to_string();
    Ok(match value {
        SubmissionProjection::Qrz(value) => {
            let contact = serde_json::to_string(&value.fields)?;
            sqlx::query_file!(
                "sql/service/qrz_projection_insert.sql",
                payload,
                account,
                destination,
                contact
            )
            .fetch_one(connection)
            .await?
            .projection_id
        }
        SubmissionProjection::Lotw(value) => {
            let external =
                identity::external(connection, key.account, &value.external_key()).await?;
            let contact = serde_json::to_string(value.contact_fields())?;
            let station = serde_json::to_string(value.station_fields())?;
            sqlx::query_file!(
                "sql/service/lotw_projection_insert.sql",
                payload,
                account,
                destination,
                value.mapping_provenance().release,
                value.mapping_provenance().upstream_commit,
                value.signing_identity().callsign.as_str(),
                value.signing_identity().dxcc,
                external,
                contact,
                station
            )
            .fetch_one(connection)
            .await?
            .projection_id
        }
    })
}

/// Restore every historical mapping and signing-input field exactly as stored.
async fn load_projection(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    projection: i64,
) -> anyhow::Result<SubmissionProjection> {
    let account_text = account.to_string();
    let row = sqlx::query_file!(
        "sql/service/projection_select.sql",
        projection,
        payload,
        account_text
    )
    .fetch_one(&mut *connection)
    .await?;
    let destination = row.destination_id.parse()?;
    Ok(match row.service.as_str() {
        "qrz"
            if row.mapping_release.is_none()
                && row.mapping_commit.is_none()
                && row.signing_call.is_none()
                && row.signing_dxcc.is_none()
                && row.external_id.is_none()
                && row.station_fields.is_none() =>
        {
            SubmissionProjection::Qrz(QrzSubmission {
                destination,
                fields: serde_json::from_str(&row.contact_fields)?,
            })
        }
        "lotw" => {
            let external = identity::load_external(
                connection,
                account,
                row.external_id.context("LoTW projection lacks exact key")?,
            )
            .await?;
            anyhow::ensure!(
                external.destination == destination,
                "LoTW projection crosses destination"
            );
            let RemoteRecordId::Lotw(key) = external.record else {
                anyhow::bail!("LoTW projection has QRZ identity")
            };
            SubmissionProjection::Lotw(Box::new(
                LotwSubmissionFields {
                    destination,
                    mapping_provenance: LotwMappingProvenance {
                        release: row
                            .mapping_release
                            .context("LoTW projection lacks mapping release")?,
                        upstream_commit: row
                            .mapping_commit
                            .context("LoTW projection lacks mapping commit")?,
                    },
                    signing_identity: PublicSigningIdentity {
                        callsign: RecordedCallsign::new(
                            &row.signing_call
                                .context("LoTW projection lacks signing call")?,
                        )?,
                        dxcc: row
                            .signing_dxcc
                            .context("LoTW projection lacks signing entity")?,
                    },
                    key,
                    contact_fields: serde_json::from_str(&row.contact_fields)?,
                    station_fields: serde_json::from_str(
                        &row.station_fields
                            .context("LoTW projection lacks station fields")?,
                    )?,
                }
                .try_into()?,
            ))
        }
        _ => anyhow::bail!("invalid submission projection variant"),
    })
}

/// Persist actual submission attempts and independent outcome evidence.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
    value: &ServicePayload,
) -> anyhow::Result<()> {
    let account = key.account.to_string();
    match value {
        ServicePayload::Attempt(value) => {
            reference(connection, key, RecordId::Qso(value.qso)).await?;
            let projection = projection(connection, key, payload, &value.projection).await?;
            let qso = value.qso.to_string();
            let phase = phase(value.phase);
            let seconds = value.at.unix_seconds();
            let nanos = i64::from(value.at.subsecond_nanoseconds());
            let batch = value.batch.map(|id| id.to_string());
            sqlx::query_file!(
                "sql/service/attempt_insert.sql",
                payload,
                account,
                qso,
                projection,
                phase,
                seconds,
                nanos,
                batch
            )
            .execute(connection)
            .await?;
        }
        ServicePayload::Result(value) => {
            reference(
                connection,
                key,
                RecordId::Service(ServiceRecordId::Destination(value.destination)),
            )
            .await?;
            let destination = value.destination.to_string();
            let (scope, batch) = match &value.scope {
                ResultScope::IndividualAttempt(_) => ("individual", None),
                ResultScope::UploadBatch { batch, .. } => ("batch", Some(batch.to_string())),
            };
            let (outcome, detail) = match &value.outcome {
                SubmissionOutcome::Success => ("success", None),
                SubmissionOutcome::Rejected(reason) => ("rejected", Some(reason.as_str())),
                SubmissionOutcome::Uncertain(reason) => ("uncertain", Some(reason.as_str())),
            };
            let phase = phase(value.phase);
            let seconds = value.at.unix_seconds();
            let nanos = i64::from(value.at.subsecond_nanoseconds());
            let external = match &value.external {
                Some(external) => {
                    Some(identity::external(connection, key.account, external).await?)
                }
                None => None,
            };
            if let Some(attempt) = value.external_attempt {
                reference(
                    connection,
                    key,
                    RecordId::Service(ServiceRecordId::Attempt(attempt)),
                )
                .await?;
            }
            let external_attempt = value.external_attempt.map(|id| id.to_string());
            sqlx::query_file!(
                "sql/service/result_insert.sql",
                payload,
                account,
                destination,
                scope,
                batch,
                outcome,
                detail,
                phase,
                seconds,
                nanos,
                value.diagnostic,
                external,
                external_attempt
            )
            .execute(&mut *connection)
            .await?;
            for attempt in value.scope.attempts() {
                reference(
                    connection,
                    key,
                    RecordId::Service(ServiceRecordId::Attempt(attempt)),
                )
                .await?;
                let attempt = attempt.to_string();
                sqlx::query_file!("sql/service/member_insert.sql", payload, account, attempt)
                    .execute(&mut *connection)
                    .await?;
            }
        }
        _ => anyhow::bail!("not submission history"),
    }
    Ok(())
}

/// Restore attempt and result scope without interpreting missing responses as failure or success.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<ServicePayload> {
    let account = key.account.to_string();
    Ok(match key.id {
        RecordId::Service(ServiceRecordId::Attempt(id)) => {
            let row = sqlx::query_file!("sql/service/attempt_select.sql", payload, account)
                .fetch_one(&mut *connection)
                .await?;
            ServicePayload::Attempt(SubmissionAttempt {
                id,
                qso: row.qso_id.parse()?,
                projection: load_projection(connection, key.account, payload, row.projection_id)
                    .await?,
                phase: read_phase(&row.phase)?,
                at: UtcTimestamp::new(row.at_seconds, row.at_nanos.try_into()?)?,
                batch: row.batch_id.map(|id| id.parse()).transpose()?,
            })
        }
        RecordId::Service(ServiceRecordId::Result(id)) => {
            let row = sqlx::query_file!("sql/service/result_select.sql", payload, account)
                .fetch_one(&mut *connection)
                .await?;
            let mut members = std::collections::BTreeSet::new();
            for member in sqlx::query_file!("sql/service/member_select.sql", payload)
                .fetch_all(&mut *connection)
                .await?
            {
                anyhow::ensure!(
                    members.insert(member.attempt_id.parse()?),
                    "duplicate result member"
                );
            }
            let scope = match (row.scope.as_str(), row.batch_id) {
                ("individual", None) if members.len() == 1 => ResultScope::IndividualAttempt(
                    members
                        .into_iter()
                        .next()
                        .context("individual result lacks member")?,
                ),
                ("batch", Some(batch)) if !members.is_empty() => ResultScope::UploadBatch {
                    batch: batch.parse()?,
                    members,
                },
                _ => anyhow::bail!("invalid result scope or member set"),
            };
            let outcome = match (row.outcome.as_str(), row.outcome_detail) {
                ("success", None) => SubmissionOutcome::Success,
                ("rejected", Some(reason)) => SubmissionOutcome::Rejected(reason),
                ("uncertain", Some(reason)) => SubmissionOutcome::Uncertain(reason),
                _ => anyhow::bail!("invalid submission outcome fields"),
            };
            let external = match row.external_id {
                Some(external) => {
                    Some(identity::load_external(connection, key.account, external).await?)
                }
                None => None,
            };
            ServicePayload::Result(SubmissionResult {
                id,
                destination: row.destination_id.parse()?,
                scope,
                outcome,
                phase: read_phase(&row.phase)?,
                at: UtcTimestamp::new(row.at_seconds, row.at_nanos.try_into()?)?,
                diagnostic: row.diagnostic,
                external,
                external_attempt: row.external_attempt.map(|id| id.parse()).transpose()?,
            })
        }
        _ => anyhow::bail!("not submission history identity"),
    })
}
