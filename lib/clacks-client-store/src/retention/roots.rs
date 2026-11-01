//! Native liveness proofs for local accepted bodies and service support.

use crate::metadata::{client, clock};
use clacks_model::ids::AccountId;
use clacks_sync::{
    record::{Position, RecordKey, RecordState, RecordValue, VersionStamp},
    service::{SupportRequirement, VersionReference, dependency::represents},
    transfer::RecordRequirement,
};
use sqlx::SqliteConnection;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Ephemeral roots proved inside the transaction performing deletion.
pub(super) struct Roots {
    /// Accepted representations whose native bodies remain in use.
    pub(super) envelopes: Vec<i64>,
}

/// Stored body identity and its native descriptor, grouped by record for support lookup.
struct Envelope {
    /// Concrete representation to retain when its native fact/effect is needed.
    id: i64,
    /// Complete native ordering and deletion descriptor.
    descriptor: RecordRequirement,
}

/// Trace actual body consumers without turning missing ordering into a successor.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    owner: AccountId,
) -> anyhow::Result<Roots> {
    let interpretation = crate::synchronization::evidence(connection, owner).await?;

    let mut envelopes: BTreeMap<RecordKey, Vec<Envelope>> = BTreeMap::new();
    let mut live = BTreeSet::new();

    for row in sqlx::query_file!("sql/retention/envelopes.sql", owner.to_string())
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account: owner,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };

        if row.rooted != 0 {
            live.insert(row.envelope_id);
        }

        envelopes.entry(key).or_default().push(Envelope {
            id: row.envelope_id,
            descriptor: RecordRequirement {
                key,
                deleted: row.deleted != 0,
                stamp: VersionStamp::new(
                    Position {
                        timeline: row.timeline_id.parse()?,
                        revision: row.revision.try_into()?,
                    },
                    row.baseline_id.parse()?,
                    serde_json::from_str(&row.body)?,
                )?,
            },
        });
    }

    let mut requirements = VecDeque::new();
    for row in sqlx::query_file!(
        "sql/retention/local_service_payloads.sql",
        owner.to_string()
    )
    .fetch_all(&mut *connection)
    .await?
    {
        let key = RecordKey {
            account: owner,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        if let RecordState::Live(RecordValue::Service(service)) =
            crate::payload::load(connection, key, row.payload_id).await?
        {
            requirements.extend(service.support.required);
        }
    }

    for row in sqlx::query_file!("sql/retention/coupled.sql", owner.to_string())
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account: owner,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        requirements.push_back(SupportRequirement::CoveredEffect(VersionReference {
            key,
            stamp: clock::load_stamp(connection, key, row.stamp_id).await?,
            deleted: row.deleted != 0,
        }));
    }

    let mut services: VecDeque<_> = envelopes
        .values()
        .flatten()
        .filter(|envelope| {
            live.contains(&envelope.id)
                && matches!(
                    envelope.descriptor.key.id,
                    clacks_sync::record::RecordId::Service(_)
                )
        })
        .collect();
    let mut inspected = BTreeSet::new();

    loop {
        while let Some(envelope) = services.pop_front() {
            if !inspected.insert(envelope.id) {
                continue;
            }

            let value = client::observation::load_envelope(
                connection,
                envelope.descriptor.key,
                envelope.id,
            )
            .await?;
            if let RecordState::Live(RecordValue::Service(service)) = value.state() {
                requirements.extend(service.support.required.iter().cloned());
            }
        }

        let Some(requirement) = requirements.pop_front() else {
            break;
        };

        let reference = requirement.reference();
        let Some(candidates) = envelopes.get(&reference.key) else {
            continue;
        };

        let exact = candidates.iter().find(|candidate| {
            candidate.descriptor.stamp == reference.stamp
                && candidate.descriptor.deleted == reference.deleted
        });
        let mut needed = if matches!(requirement, SupportRequirement::ExactFact(_)) {
            exact
        } else {
            None
        };

        if needed.is_none() {
            for candidate in candidates
                .iter()
                .filter(|candidate| live.contains(&candidate.id))
                .chain(candidates.iter())
            {
                if represents(&requirement, &candidate.descriptor, &interpretation.view())? {
                    needed = Some(candidate);
                    break;
                }
            }
        }

        if let Some(envelope) = needed.or(exact)
            && live.insert(envelope.id)
            && matches!(
                envelope.descriptor.key.id,
                clacks_sync::record::RecordId::Service(_)
            )
        {
            services.push_back(envelope);
        }
    }

    Ok(Roots {
        envelopes: live.into_iter().collect(),
    })
}
