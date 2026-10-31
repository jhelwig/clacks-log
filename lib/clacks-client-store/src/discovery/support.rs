//! Exact immutable support and atomic-unit membership for accepted-state return.

use crate::metadata::{client, clock};
use clacks_sync::{
    record::{AcceptedRecord, Observation, RecordKey, RecordState, RecordValue},
    service::{VersionReference, operation::CoupledRequirement},
};
use sqlx::SqliteConnection;
use std::collections::BTreeSet;

/// Include retained dependency and coupled-effect closure from the same read snapshot.
pub(super) async fn complete(
    connection: &mut SqliteConnection,
    owner: clacks_model::ids::AccountId,
    heads: &mut Vec<AcceptedRecord>,
    support: &mut Vec<AcceptedRecord>,
    require_complete: bool,
) -> anyhow::Result<Vec<CoupledRequirement>> {
    let mut values = heads.clone();
    values.extend(support.iter().cloned());
    let mut index = 0;
    let mut units = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some(value) = values.get(index).cloned() {
        index += 1;
        let key = value.key();
        let mut keys = clacks_sync::acceptance::required_references(key, value.state());
        let mut references = Vec::new();
        if let RecordState::Live(RecordValue::Service(service)) = value.state() {
            keys.extend(clacks_sync::service::dependency::required_keys(
                owner, service,
            ));
            references.extend(
                service
                    .support
                    .required
                    .iter()
                    .map(|item| item.reference().clone()),
            );
        }
        for row in sqlx::query_file!(
            "sql/discovery/units.sql",
            owner.to_string(),
            crate::payload::kind(key.id),
            key.id.to_string()
        )
        .fetch_all(&mut *connection)
        .await?
        {
            if seen.insert(row.operation_id.clone()) {
                let mut members = Vec::new();
                for member in sqlx::query_file!(
                    "sql/discovery/members.sql",
                    owner.to_string(),
                    row.operation_id
                )
                .fetch_all(&mut *connection)
                .await?
                {
                    let key = RecordKey {
                        account: owner,
                        id: crate::payload::record_id(&member.kind, &member.record_id)?,
                    };
                    members.push(VersionReference {
                        key,
                        stamp: clock::load_stamp(connection, key, member.stamp_id).await?,
                        deleted: member.deleted != 0,
                    });
                }
                references.extend(members.clone());
                units.push(CoupledRequirement {
                    id: row.operation_id.parse()?,
                    members,
                });
            }
        }
        for key in keys {
            if !heads.iter().any(|value| value.key() == key)
                && let Some(row) = client::load(connection, key).await?
                && let Observation::Accepted(value) = row.latest()
            {
                heads.push(value.clone());
                if !values.contains(value) {
                    values.push(value.clone());
                }
            }
        }
        for reference in references {
            if !values
                .iter()
                .any(|value| value.key() == reference.key && value.stamp == reference.stamp)
            {
                match client::observation::exact(connection, &reference).await? {
                    Some(value) => {
                        support.push(value.clone());
                        values.push(value);
                    }
                    None if require_complete => {
                        anyhow::bail!("required accepted support unavailable")
                    }
                    None => {}
                }
            }
        }
    }
    support.retain(|value| !heads.contains(value));
    Ok(units)
}
