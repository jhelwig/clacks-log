//! Retained provider observations with source coverage and acquisition provenance.

use super::{acquisition, identity, reference, report};
use crate::store::rows::service::RemoteIdentityRow;
use anyhow::Context;
use clacks_model::{
    scalar::{UtcSecond, UtcTimestamp},
    service::evidence::{
        ServiceRecordId,
        observation::{
            AcquisitionProvenance, ProviderOrdering, RemoteFacts, RemoteIdentity, RemoteObservation,
        },
    },
};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::PgConnection;

/// Store a meaningful report without turning missing identity fields into a guessed external key.
pub(super) async fn insert(
    connection: &mut PgConnection,
    key: RecordKey,
    payload: i64,
    value: &RemoteObservation,
) -> anyhow::Result<()> {
    reference(
        connection,
        key,
        RecordId::Service(ServiceRecordId::Destination(value.destination)),
    )
    .await?;
    reference(
        connection,
        key,
        RecordId::Service(ServiceRecordId::Acquisition(value.acquisition.id)),
    )
    .await?;
    let contact = report::contact(connection, payload, &value.facts.contact).await?;
    let own = report::station(connection, key.account, payload, &value.facts.own_station).await?;
    let counterpart = match &value.facts.counterpart {
        Some(report) => Some(report::station(connection, key.account, payload, report).await?),
        None => None,
    };
    let query =
        acquisition::query(connection, key.account, payload, &value.acquisition.query).await?;
    let account = key.account.to_string();
    let destination = value.destination.to_string();
    let acquisition = value.acquisition.id.to_string();
    let qso_order = value.ordering.qso.map(UtcSecond::unix_seconds);
    let confirmation_order = value.ordering.confirmation.map(UtcSecond::unix_seconds);
    let seconds = value.acquisition.observed_at.unix_seconds();
    let nanos = i64::from(value.acquisition.observed_at.subsecond_nanoseconds());
    match &value.identity {
        RemoteIdentity::Complete(external) => {
            let external = identity::external(connection, key.account, external).await?;
            sqlx::query_file!(
                "sql/persistence/service/observation_insert.sql",
                payload,
                account,
                destination,
                "complete",
                external,
                None::<String>,
                None::<String>,
                contact,
                own,
                counterpart,
                qso_order,
                confirmation_order,
                acquisition,
                query,
                seconds,
                nanos
            )
            .execute(connection)
            .await?;
        }
        RemoteIdentity::Incomplete { service, available } => {
            let available = serde_json::to_string(available)?;
            sqlx::query_file!(
                "sql/persistence/service/observation_insert.sql",
                payload,
                account,
                destination,
                "incomplete",
                None::<i64>,
                identity::service(*service),
                available,
                contact,
                own,
                counterpart,
                qso_order,
                confirmation_order,
                acquisition,
                query,
                seconds,
                nanos
            )
            .execute(connection)
            .await?;
        }
    }
    Ok(())
}

/// Restore independently scoped report and provider-ordering information exactly.
pub(super) async fn load(
    connection: &mut PgConnection,
    key: RecordKey,
    payload: i64,
) -> anyhow::Result<RemoteObservation> {
    let RecordId::Service(ServiceRecordId::Observation(id)) = key.id else {
        anyhow::bail!("not observation identity")
    };
    let account = key.account.to_string();
    let row = sqlx::query_file!(
        "sql/persistence/service/observation_select.sql",
        payload,
        account
    )
    .fetch_one(&mut *connection)
    .await?;
    let identity = match (RemoteIdentityRow {
        identity: row.identity.as_str(),
        external_id: row.external_id,
        incomplete_service: row.incomplete_service.as_deref(),
        available_fields: row.available_fields.as_deref(),
    }) {
        RemoteIdentityRow {
            identity: "complete",
            external_id: Some(external),
            incomplete_service: None,
            available_fields: None,
        } => RemoteIdentity::Complete(
            identity::load_external(connection, key.account, external).await?,
        ),
        RemoteIdentityRow {
            identity: "incomplete",
            external_id: None,
            incomplete_service: Some(service),
            available_fields: Some(available),
        } => RemoteIdentity::Incomplete {
            service: identity::read_service(service)?,
            available: serde_json::from_str(available)?,
        },
        _ => anyhow::bail!("invalid remote identity variant"),
    };
    let contact = report::load_contact(connection, payload, row.contact_report).await?;
    let own_station =
        report::load_station(connection, key.account, payload, row.own_report).await?;
    let counterpart = match row.counterpart_report {
        Some(report) => Some(report::load_station(connection, key.account, payload, report).await?),
        None => None,
    };
    let query = acquisition::load_query(connection, key.account, payload, row.query_id).await?;
    Ok(RemoteObservation {
        id,
        destination: row.destination_id.parse()?,
        identity,
        facts: RemoteFacts {
            contact,
            own_station,
            counterpart,
        },
        ordering: ProviderOrdering {
            qso: row.qso_order.map(UtcSecond::from_unix_seconds),
            confirmation: row.confirmation_order.map(UtcSecond::from_unix_seconds),
        },
        acquisition: AcquisitionProvenance {
            id: row
                .acquisition_id
                .parse()
                .context("invalid observation acquisition ID")?,
            query,
            observed_at: UtcTimestamp::new(row.observed_seconds, row.observed_nanos.try_into()?)?,
        },
    })
}
