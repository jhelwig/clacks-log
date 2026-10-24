//! Original comparison bases and retained compaction witnesses.

use super::observation::{insert_envelope, load_envelope, read_unknown, unknown};
use crate::{
    metadata::{clock, lineage},
    rows::client::OriginalBasisRow,
};
use clacks_sync::{
    client::OriginalBasis, compaction::BasisWitness, compare::RecordRelation, record::RecordKey,
};
use sqlx::SqliteConnection;

/// Store genuine original values and their existing comparison evidence.
pub(crate) async fn insert_original(
    connection: &mut SqliteConnection,
    value: &OriginalBasis,
) -> anyhow::Result<i64> {
    let key = value.key();
    crate::payload::identity(connection, key).await?;
    let row = match value {
        OriginalBasis::Accepted(value) => OriginalBasisRow {
            variant: "accepted".into(),
            envelope_id: Some(insert_envelope(connection, value).await?),
            checkpoint_id: None,
            payload_id: None,
            vector_id: None,
            stamp_id: None,
            unknown_reason: None,
        },
        OriginalBasis::NeverCreated { checkpoint, .. } => OriginalBasisRow {
            variant: "absent".into(),
            envelope_id: None,
            checkpoint_id: Some(lineage::insert_checkpoint(connection, checkpoint).await?),
            payload_id: None,
            vector_id: None,
            stamp_id: None,
            unknown_reason: None,
        },
        OriginalBasis::SubsumedOwn {
            state,
            vector,
            witness,
            ..
        } => OriginalBasisRow {
            variant: "subsumed".into(),
            envelope_id: None,
            checkpoint_id: None,
            payload_id: Some(crate::payload::insert(connection, key, state).await?),
            vector_id: Some(clock::insert_vector(connection, key, vector).await?),
            stamp_id: Some(clock::insert_stamp(connection, key, witness).await?),
            unknown_reason: None,
        },
        OriginalBasis::Unavailable { reason, .. } => OriginalBasisRow {
            variant: "unavailable".into(),
            envelope_id: None,
            checkpoint_id: None,
            payload_id: None,
            vector_id: None,
            stamp_id: None,
            unknown_reason: Some(unknown(*reason).into()),
        },
    };
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    Ok(sqlx::query_file!(
        "sql/original/insert.sql",
        account,
        kind,
        id,
        row.variant,
        row.envelope_id,
        row.checkpoint_id,
        row.payload_id,
        row.vector_id,
        row.stamp_id,
        row.unknown_reason
    )
    .fetch_one(connection)
    .await?
    .original_id)
}

/// Restore all original-basis alternatives without relabeling their retained contexts.
pub(crate) async fn load_original(
    connection: &mut SqliteConnection,
    key: RecordKey,
    original: i64,
) -> anyhow::Result<OriginalBasis> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file_as!(
        OriginalBasisRow,
        "sql/original/select.sql",
        original,
        account,
        kind,
        id
    )
    .fetch_one(&mut *connection)
    .await?;
    Ok(match row {
        OriginalBasisRow {
            variant,
            envelope_id: Some(envelope),
            checkpoint_id: None,
            payload_id: None,
            vector_id: None,
            stamp_id: None,
            unknown_reason: None,
        } if variant == "accepted" => {
            OriginalBasis::Accepted(load_envelope(connection, key, envelope).await?)
        }
        OriginalBasisRow {
            variant,
            envelope_id: None,
            checkpoint_id: Some(checkpoint),
            payload_id: None,
            vector_id: None,
            stamp_id: None,
            unknown_reason: None,
        } if variant == "absent" => OriginalBasis::NeverCreated {
            key,
            checkpoint: lineage::load_checkpoint(connection, key.account, checkpoint).await?,
        },
        OriginalBasisRow {
            variant,
            envelope_id: None,
            checkpoint_id: None,
            payload_id: Some(payload),
            vector_id: Some(vector),
            stamp_id: Some(stamp),
            unknown_reason: None,
        } if variant == "subsumed" => OriginalBasis::SubsumedOwn {
            key,
            state: crate::payload::load(connection, key, payload).await?,
            vector: clock::load_vector(connection, key, vector).await?,
            witness: clock::load_stamp(connection, key, stamp).await?,
        },
        OriginalBasisRow {
            variant,
            envelope_id: None,
            checkpoint_id: None,
            payload_id: None,
            vector_id: None,
            stamp_id: None,
            unknown_reason: Some(reason),
        } if variant == "unavailable" => OriginalBasis::Unavailable {
            key,
            reason: read_unknown(&reason)?,
        },
        _ => anyhow::bail!("invalid original-basis variant fields"),
    })
}

/// Store a sealed original relationship together with its private interpretation roots.
pub(super) async fn insert_witness(
    connection: &mut SqliteConnection,
    value: &BasisWitness,
    original: i64,
) -> anyhow::Result<i64> {
    let key = value.key;
    let observed = clock::insert_stamp(connection, key, &value.observed).await?;
    let checkpoint = lineage::insert_checkpoint(connection, &value.checkpoint).await?;
    let lineage = lineage::insert_lineage(connection, &value.lineage).await?;
    let relation = match value.relation {
        RecordRelation::SameVersion => "same",
        RecordRelation::Before => "before",
        _ => anyhow::bail!("witness lacks an established original relationship"),
    };
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let witness = sqlx::query_file!(
        "sql/witness/insert.sql",
        account,
        kind,
        id,
        original,
        observed,
        checkpoint,
        relation,
        lineage
    )
    .fetch_one(&mut *connection)
    .await?
    .witness_id;
    for (ordinal, baseline) in value.baselines.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let baseline = lineage::insert_baseline(connection, baseline).await?;
        sqlx::query_file!(
            "sql/witness/baseline_insert.sql",
            witness,
            ordinal,
            baseline
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(witness)
}

/// Restore self-contained comparison evidence with its original baseline and ancestry meanings.
pub(super) async fn load_witness(
    connection: &mut SqliteConnection,
    key: RecordKey,
    witness: i64,
) -> anyhow::Result<BasisWitness> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let row = sqlx::query_file!("sql/witness/select.sql", witness, account, kind, id)
        .fetch_one(&mut *connection)
        .await?;
    let original = load_original(connection, key, row.original_id).await?;
    let observed = clock::load_stamp(connection, key, row.observed_stamp).await?;
    let checkpoint = lineage::load_checkpoint(connection, key.account, row.checkpoint_id).await?;
    let lineage = lineage::load_lineage(connection, key.account, row.lineage_id).await?;
    let mut baselines = Vec::new();
    for row in sqlx::query_file!("sql/witness/baseline_select.sql", witness)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(baselines.len())?,
            "noncontiguous witness baselines"
        );
        baselines.push(lineage::load_baseline(connection, key.account, row.baseline_row).await?);
    }
    Ok(BasisWitness {
        key,
        original,
        observed,
        checkpoint,
        lineage,
        baselines,
        relation: match row.relation.as_str() {
            "same" => RecordRelation::SameVersion,
            "before" => RecordRelation::Before,
            _ => anyhow::bail!("invalid witness relationship"),
        },
    })
}
