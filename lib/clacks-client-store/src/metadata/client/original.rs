//! Original comparison bases and retained compaction witnesses.

use super::observation::{insert_envelope, load_envelope, read_unknown, unknown};
use crate::{
    metadata::{clock, lineage},
    rows::client::OriginalBasisRow,
};
use clacks_sync::{
    client::OriginalBasis,
    compaction::{BasisWitness, EstablishedBasisRelation},
    record::RecordKey,
};
use sqlx::SqliteConnection;

/// Store genuine original values and their existing comparison evidence.
pub(crate) async fn insert_original(
    connection: &mut SqliteConnection,
    value: &OriginalBasis,
) -> anyhow::Result<i64> {
    let key = value.key();
    crate::payload::identity(connection, key).await?;
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    Ok(match value {
        OriginalBasis::Accepted(value) => {
            let envelope = insert_envelope(connection, value).await?;
            sqlx::query_file!(
                "sql/original/insert.sql",
                account,
                kind,
                id,
                "accepted",
                envelope,
                None::<i64>,
                None::<i64>,
                None::<i64>,
                None::<i64>,
                None::<String>
            )
            .fetch_one(connection)
            .await?
            .original_id
        }
        OriginalBasis::NeverCreated { checkpoint, .. } => {
            let checkpoint = lineage::insert_checkpoint(connection, checkpoint).await?;
            sqlx::query_file!(
                "sql/original/insert.sql",
                account,
                kind,
                id,
                "absent",
                None::<i64>,
                checkpoint,
                None::<i64>,
                None::<i64>,
                None::<i64>,
                None::<String>
            )
            .fetch_one(connection)
            .await?
            .original_id
        }
        OriginalBasis::SubsumedOwn {
            state,
            vector,
            witness,
            ..
        } => {
            let payload = crate::payload::insert(connection, key, state).await?;
            let vector = clock::insert_vector(connection, key, vector).await?;
            let stamp = clock::insert_stamp(connection, key, witness).await?;
            sqlx::query_file!(
                "sql/original/insert.sql",
                account,
                kind,
                id,
                "subsumed",
                None::<i64>,
                None::<i64>,
                payload,
                vector,
                stamp,
                None::<String>
            )
            .fetch_one(connection)
            .await?
            .original_id
        }
        OriginalBasis::Unavailable { reason, .. } => {
            sqlx::query_file!(
                "sql/original/insert.sql",
                account,
                kind,
                id,
                "unavailable",
                None::<i64>,
                None::<i64>,
                None::<i64>,
                None::<i64>,
                None::<i64>,
                unknown(*reason)
            )
            .fetch_one(connection)
            .await?
            .original_id
        }
    })
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
    let key = value.key();
    let observed = clock::insert_stamp(connection, key, value.observed()).await?;
    let checkpoint = lineage::insert_checkpoint(connection, value.checkpoint()).await?;
    let lineage = lineage::insert_lineage(connection, value.lineage()).await?;
    let relation = match value.relation() {
        EstablishedBasisRelation::SameVersion => "same",
        EstablishedBasisRelation::Before => "before",
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
    for (ordinal, baseline) in value.baselines().iter().enumerate() {
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
    let value = BasisWitness::restore(original, observed, checkpoint, lineage, baselines)?;
    let relation = match row.relation.as_str() {
        "same" => EstablishedBasisRelation::SameVersion,
        "before" => EstablishedBasisRelation::Before,
        _ => anyhow::bail!("invalid witness relationship"),
    };
    anyhow::ensure!(
        value.relation() == relation,
        "witness relationship differs from retained evidence"
    );
    Ok(value)
}
