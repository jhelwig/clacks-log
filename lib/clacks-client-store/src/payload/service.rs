//! Immutable service evidence, maintained state and their retained support.

mod acquisition;
mod assertion;
mod identity;
mod observation;
mod projection;
mod report;

use crate::metadata::clock;
use clacks_model::service::evidence::{
    ServicePayload, ServiceRecordId,
    processing::{
        AssociationState, DeletionSuppression, ExternalAssociation, ProcessingDecision,
        ProcessingDisposition,
    },
};
use clacks_sync::{
    record::{RecordId, RecordKey},
    service::{ServiceRecord, ServiceSupport, SupportRequirement, VersionReference},
};
use sqlx::SqliteConnection;

/// Ensure a structural native dependency without inventing its accepted values.
async fn reference(
    connection: &mut SqliteConnection,
    owner: RecordKey,
    id: RecordId,
) -> anyhow::Result<()> {
    crate::payload::identity(
        connection,
        RecordKey {
            account: owner.account,
            id,
        },
    )
    .await
}

/// Store a complete service value with ordered support and frozen public descriptors.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
    value: &ServiceRecord,
) -> anyhow::Result<()> {
    let acquisition = match &value.support.acquisition {
        Some(stamp) => Some(clock::insert_stamp(connection, key, stamp).await?),
        None => None,
    };
    let descriptor = match &value.support.destination {
        Some(destination) => {
            Some(identity::descriptor(connection, key.account, payload, destination).await?)
        }
        None => None,
    };
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let operation = value.support.operation.map(|id| id.to_string());
    sqlx::query_file!(
        "sql/service/support_insert.sql",
        payload,
        account,
        kind,
        id,
        operation,
        acquisition,
        descriptor
    )
    .execute(&mut *connection)
    .await?;
    for (ordinal, requirement) in value.support.required.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let retained = requirement.reference();
        let stamp = clock::insert_stamp(connection, retained.key, &retained.stamp).await?;
        let requirement = match requirement {
            SupportRequirement::ExactFact(_) => "fact",
            SupportRequirement::CoveredEffect(_) => "effect",
        };
        let kind = crate::payload::kind(retained.key.id);
        let id = retained.key.id.to_string();
        sqlx::query_file!(
            "sql/service/requirement_insert.sql",
            payload,
            ordinal,
            requirement,
            account,
            kind,
            id,
            stamp,
            retained.deleted
        )
        .execute(&mut *connection)
        .await?;
    }
    match &value.payload {
        ServicePayload::Destination(destination) => {
            let descriptor =
                identity::descriptor(connection, key.account, payload, destination).await?;
            sqlx::query_file!("sql/service/destination_insert.sql", payload, descriptor)
                .execute(connection)
                .await?;
        }
        ServicePayload::Association(value) => {
            reference(connection, key, RecordId::Qso(value.qso)).await?;
            let external = identity::external(connection, key.account, &value.external).await?;
            let qso = value.qso.to_string();
            let state = match value.state {
                AssociationState::Active => "active",
                AssociationState::Superseded => "superseded",
                AssociationState::DeletedQso => "deleted",
            };
            sqlx::query_file!(
                "sql/service/association_insert.sql",
                payload,
                account,
                external,
                qso,
                state
            )
            .execute(connection)
            .await?;
        }
        ServicePayload::Suppression(value) => {
            reference(connection, key, RecordId::Qso(value.qso)).await?;
            let external = identity::external(connection, key.account, &value.external).await?;
            let qso = value.qso.to_string();
            sqlx::query_file!(
                "sql/service/suppression_insert.sql",
                payload,
                account,
                external,
                qso
            )
            .execute(connection)
            .await?;
        }
        ServicePayload::Disposition(value) => {
            reference(
                connection,
                key,
                RecordId::Service(ServiceRecordId::Observation(value.observation)),
            )
            .await?;
            if let Some(association) = value.association {
                reference(
                    connection,
                    key,
                    RecordId::Service(ServiceRecordId::Association(association)),
                )
                .await?;
            }
            let observation = value.observation.to_string();
            let association = value.association.map(|id| id.to_string());
            let decision = match value.decision {
                ProcessingDecision::PendingReview => "review",
                ProcessingDecision::Associated => "associated",
                ProcessingDecision::Added => "added",
                ProcessingDecision::Rejected => "rejected",
                ProcessingDecision::Suppressed => "suppressed",
                ProcessingDecision::CompletedEquivalent => "equivalent",
            };
            sqlx::query_file!(
                "sql/service/disposition_insert.sql",
                payload,
                account,
                observation,
                decision,
                association
            )
            .execute(connection)
            .await?;
        }
        ServicePayload::Download(_) | ServicePayload::Acquisition(_) => {
            acquisition::insert(connection, key, payload, &value.payload).await?
        }
        ServicePayload::Attempt(_) | ServicePayload::Result(_) => {
            projection::insert(connection, key, payload, &value.payload).await?
        }
        ServicePayload::Observation(value) => {
            observation::insert(connection, key, payload, value).await?
        }
        ServicePayload::Imported(value) => {
            assertion::insert(connection, key, payload, value).await?
        }
    }
    Ok(())
}

/// Restore a complete service carrier without rerunning projections or provider requests.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<ServiceRecord> {
    let roots = sqlx::query_file!("sql/service/domain_count.sql", payload)
        .fetch_one(&mut *connection)
        .await?
        .roots;
    anyhow::ensure!(
        roots == 1,
        "service payload must have exactly one domain root"
    );
    let orphans = sqlx::query_file!("sql/service/orphan_children.sql", payload)
        .fetch_one(&mut *connection)
        .await?
        .orphans;
    anyhow::ensure!(
        orphans == 0,
        "service payload contains unreferenced semantic children"
    );
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file!("sql/service/support_select.sql", payload, account, kind, id)
        .fetch_one(&mut *connection)
        .await?;
    let acquisition = match row.acquisition_stamp {
        Some(stamp) => Some(clock::load_stamp(connection, key, stamp).await?),
        None => None,
    };
    let destination = match row.descriptor_id {
        Some(descriptor) => {
            Some(identity::load_descriptor(connection, key.account, payload, descriptor).await?)
        }
        None => None,
    };
    let mut required = Vec::new();
    for row in sqlx::query_file!("sql/service/requirement_select.sql", payload)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            i64::try_from(required.len())? == row.ordinal
                && row.account == account
                && matches!(row.deleted, 0 | 1),
            "invalid service support scope or sequence"
        );
        let key = RecordKey {
            account: row.account.parse()?,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        let reference = VersionReference {
            key,
            stamp: clock::load_stamp(connection, key, row.stamp_id).await?,
            deleted: row.deleted != 0,
        };
        required.push(match row.requirement.as_str() {
            "fact" => SupportRequirement::ExactFact(reference),
            "effect" => SupportRequirement::CoveredEffect(reference),
            _ => anyhow::bail!("unknown service requirement"),
        });
    }
    let value = match key.id {
        RecordId::Service(ServiceRecordId::Destination(_)) => {
            let row = sqlx::query_file!("sql/service/destination_select.sql", payload)
                .fetch_one(&mut *connection)
                .await?;
            ServicePayload::Destination(
                identity::load_descriptor(connection, key.account, payload, row.descriptor_id)
                    .await?,
            )
        }
        RecordId::Service(ServiceRecordId::Association(id)) => {
            let row = sqlx::query_file!("sql/service/association_select.sql", payload, account)
                .fetch_one(&mut *connection)
                .await?;
            ServicePayload::Association(ExternalAssociation {
                id,
                external: identity::load_external(connection, key.account, row.external_id).await?,
                qso: row.qso_id.parse()?,
                state: match row.state.as_str() {
                    "active" => AssociationState::Active,
                    "superseded" => AssociationState::Superseded,
                    "deleted" => AssociationState::DeletedQso,
                    _ => anyhow::bail!("unknown association state"),
                },
            })
        }
        RecordId::Service(ServiceRecordId::Suppression(id)) => {
            let row = sqlx::query_file!("sql/service/suppression_select.sql", payload, account)
                .fetch_one(&mut *connection)
                .await?;
            ServicePayload::Suppression(DeletionSuppression {
                id,
                external: identity::load_external(connection, key.account, row.external_id).await?,
                qso: row.qso_id.parse()?,
            })
        }
        RecordId::Service(ServiceRecordId::Disposition(id)) => {
            let row = sqlx::query_file!("sql/service/disposition_select.sql", payload, account)
                .fetch_one(connection)
                .await?;
            ServicePayload::Disposition(ProcessingDisposition {
                id,
                observation: row.observation_id.parse()?,
                association: row.association_id.map(|id| id.parse()).transpose()?,
                decision: match row.decision.as_str() {
                    "review" => ProcessingDecision::PendingReview,
                    "associated" => ProcessingDecision::Associated,
                    "added" => ProcessingDecision::Added,
                    "rejected" => ProcessingDecision::Rejected,
                    "suppressed" => ProcessingDecision::Suppressed,
                    "equivalent" => ProcessingDecision::CompletedEquivalent,
                    _ => anyhow::bail!("unknown processing decision"),
                },
            })
        }
        RecordId::Service(ServiceRecordId::Download(_) | ServiceRecordId::Acquisition(_)) => {
            acquisition::load(connection, key, payload).await?
        }
        RecordId::Service(ServiceRecordId::Attempt(_) | ServiceRecordId::Result(_)) => {
            projection::load(connection, key, payload).await?
        }
        RecordId::Service(ServiceRecordId::Observation(_)) => ServicePayload::Observation(
            Box::new(observation::load(connection, key, payload).await?),
        ),
        RecordId::Service(ServiceRecordId::Assertion(_)) => {
            ServicePayload::Imported(Box::new(assertion::load(connection, key, payload).await?))
        }
        _ => anyhow::bail!("not a service identity"),
    };
    let record = ServiceRecord {
        payload: value,
        support: ServiceSupport {
            required,
            operation: row.operation_id.map(|id| id.parse()).transpose()?,
            acquisition,
            destination,
        },
    };
    Ok(record)
}

#[cfg(test)]
mod tests;
