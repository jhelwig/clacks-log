//! Exact support and coupled effects accompanying accepted discovery input.

use crate::store::{metadata, synchronization::context};
use clacks_sync::{
    account::AccountMetadata,
    record::{AcceptedRecord, Observation, RecordState, RecordValue},
    service::{VersionReference, operation::CoupledRequirement},
};
use sqlx::PgConnection;
use std::collections::BTreeSet;

/// Include every exact service prerequisite and coupled member reachable from this page.
///
/// Complete discovery rejects missing exact support; returned facts may retain an incomplete closure for staging.
pub(crate) async fn load(
    connection: &mut PgConnection,
    account: &AccountMetadata,
    heads: &mut Vec<AcceptedRecord>,
    support: &mut Vec<AcceptedRecord>,
    require_complete: bool,
) -> anyhow::Result<Vec<CoupledRequirement>> {
    let mut values = heads.clone();
    values.extend(support.iter().cloned());
    let mut units = Vec::new();
    let mut known_units = BTreeSet::new();
    let mut index = 0;
    while let Some(value) = values.get(index).cloned() {
        index += 1;
        let key = value.key();
        let mut exact = Vec::new();
        let mut keys = clacks_sync::acceptance::required_references(key, value.state());
        if let RecordState::Live(RecordValue::Service(service)) = value.state() {
            keys.extend(clacks_sync::service::dependency::required_keys(
                account.account,
                service,
            ));
            exact.extend(
                service
                    .support
                    .required
                    .iter()
                    .map(|item| item.reference().clone()),
            );
        }
        for row in sqlx::query_file!(
            "sql/discovery/units.sql",
            account.account.to_string(),
            crate::store::payload::kind(key.id),
            key.id.to_string()
        )
        .fetch_all(&mut *connection)
        .await?
        {
            if known_units.insert(row.operation_id.clone()) {
                let mut members = Vec::new();
                for member in sqlx::query_file!(
                    "sql/discovery/members.sql",
                    account.account.to_string(),
                    row.operation_id
                )
                .fetch_all(&mut *connection)
                .await?
                {
                    let key = clacks_sync::record::RecordKey {
                        account: account.account,
                        id: crate::store::payload::record_id(&member.kind, &member.record_id)?,
                    };
                    members.push(VersionReference {
                        key,
                        stamp: metadata::clock::load_stamp(connection, key, member.stamp_id)
                            .await?,
                        deleted: member.deleted,
                    });
                }
                exact.extend(members.clone());
                units.push(CoupledRequirement {
                    id: row.operation_id.parse()?,
                    members,
                });
            }
        }
        for key in keys {
            if !heads.iter().any(|value| value.key() == key)
                && let Observation::Accepted(value) =
                    context::observation(connection, account, key).await?
            {
                heads.push(value.clone());
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
        for reference in exact {
            if !values
                .iter()
                .any(|value| value.key() == reference.key && value.stamp == reference.stamp)
            {
                match context::exact(connection, &reference).await? {
                    Some(value) => {
                        support.push(value.clone());
                        values.push(value);
                    }
                    None if require_complete => {
                        anyhow::bail!(super::SourceError::MissingSupport)
                    }
                    None => {}
                }
            }
        }
    }
    support.retain(|value| !heads.contains(value));
    Ok(units)
}
