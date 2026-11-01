//! Native liveness proofs for historical bodies and retained ordering descriptions.

use crate::store::{
    metadata::{account, clock},
    payload,
};
use clacks_model::ids::AccountId;
use clacks_sync::{
    compaction::installation::common_coverage,
    record::{Position, RecordKey, RecordState, RecordValue, VersionStamp},
    service::{SupportRequirement, VersionReference, dependency::represents},
    transfer::RecordRequirement,
};
use sqlx::PgConnection;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

/// Ephemeral roots proved inside the transaction performing deletion.
pub(super) struct Roots {
    /// Accepted representations whose native bodies remain in use.
    pub(super) envelopes: Vec<i64>,
    /// Ordering descriptions above common coverage, independent of payload use.
    pub(super) stamps: Vec<i64>,
}

/// Stored body identity and its native descriptor, grouped by record for support lookup.
struct Envelope {
    /// Concrete representation to retain when its native fact/effect is needed.
    id: i64,
    /// Complete native ordering and deletion descriptor.
    descriptor: RecordRequirement,
}

/// Trace actual body consumers without turning missing ordering into a successor.
pub(super) async fn load(connection: &mut PgConnection, owner: AccountId) -> anyhow::Result<Roots> {
    let current = account::load(connection, owner).await?;
    let membership = crate::enrollment::membership::snapshot(connection, &current).await?;
    let interpretation = account::interpretation(connection, owner).await?;
    let common = common_coverage(&current, &membership, &interpretation.view())?;

    let mut envelopes: BTreeMap<RecordKey, Vec<Envelope>> = BTreeMap::new();
    let mut live = BTreeSet::new();

    for row in sqlx::query_file!("sql/retention/envelopes.sql", owner.to_string())
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account: owner,
            id: payload::record_id(&row.kind, &row.record_id)?,
        };

        if row.rooted {
            live.insert(row.envelope_id);
        }

        envelopes.entry(key).or_default().push(Envelope {
            id: row.envelope_id,
            descriptor: RecordRequirement {
                key,
                deleted: row.deleted,
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
    for row in sqlx::query_file!("sql/retention/coupled.sql", owner.to_string())
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account: owner,
            id: payload::record_id(&row.kind, &row.record_id)?,
        };
        requirements.push_back(SupportRequirement::CoveredEffect(VersionReference {
            key,
            stamp: clock::load_stamp(connection, key, row.stamp_id).await?,
            deleted: row.deleted,
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

            let value = crate::store::metadata::accepted::load_envelope(
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

    let mut protected = Vec::new();
    let mut seen: BTreeMap<RecordKey, HashSet<VersionStamp>> = BTreeMap::new();
    for row in sqlx::query_file!("sql/retention/stamps.sql", owner.to_string())
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account: owner,
            id: payload::record_id(&row.kind, &row.record_id)?,
        };
        let stamp = VersionStamp::new(
            Position {
                timeline: row.timeline_id.parse()?,
                revision: row.revision.try_into()?,
            },
            row.baseline_id.parse()?,
            serde_json::from_str(&row.body)?,
        )?;
        if !common.covers(stamp.accepted_at()) && seen.entry(key).or_default().insert(stamp) {
            protected.push(row.stamp_id);
        }
    }

    Ok(Roots {
        envelopes: live.into_iter().collect(),
        stamps: protected,
    })
}
