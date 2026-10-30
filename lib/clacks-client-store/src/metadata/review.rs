//! Durable client-owned resolution context returned by native transitions.

use super::client::{observation, original};
use crate::rows::review::ReviewReasonRow;
use clacks_sync::{
    client::{ReviewInput, ReviewReason},
    record::RecordKey,
};
use sqlx::SqliteConnection;

/// Replace shown resolution context with the actual result of a native transition.
pub(crate) async fn store(
    connection: &mut SqliteConnection,
    key: RecordKey,
    review: Option<&ReviewInput>,
) -> anyhow::Result<()> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    sqlx::query_file!("sql/review/delete.sql", account, kind, id)
        .execute(&mut *connection)
        .await?;
    let Some(review) = review else { return Ok(()) };
    anyhow::ensure!(
        review.original.key() == key && review.shared.key() == key,
        "review crosses record scope"
    );
    let original = original::insert_original(connection, &review.original).await?;
    let payload = crate::payload::insert(connection, key, &review.local).await?;
    let shared = observation::insert_observation(connection, &review.shared).await?;
    match &review.reason {
        ReviewReason::BothDifferent => {
            sqlx::query_file!(
                "sql/review/insert.sql",
                account,
                kind,
                id,
                original,
                payload,
                shared,
                "different",
                None::<String>,
                None::<String>,
                None::<String>,
                None::<String>
            )
            .execute(&mut *connection)
            .await?;
        }
        ReviewReason::MissingOriginal => {
            sqlx::query_file!(
                "sql/review/insert.sql",
                account,
                kind,
                id,
                original,
                payload,
                shared,
                "original",
                None::<String>,
                None::<String>,
                None::<String>,
                None::<String>
            )
            .execute(&mut *connection)
            .await?;
        }
        ReviewReason::OrderingUnknown(value) => {
            sqlx::query_file!(
                "sql/review/insert.sql",
                account,
                kind,
                id,
                original,
                payload,
                shared,
                "ordering",
                observation::unknown(*value),
                None::<String>,
                None::<String>,
                None::<String>
            )
            .execute(&mut *connection)
            .await?;
        }
        ReviewReason::MissingReference(value) => {
            anyhow::ensure!(
                value.account == key.account,
                "review reference crosses account"
            );
            crate::payload::identity(connection, *value).await?;
            sqlx::query_file!(
                "sql/review/insert.sql",
                account,
                kind,
                id,
                original,
                payload,
                shared,
                "reference",
                None::<String>,
                crate::payload::kind(value.id),
                value.id.to_string(),
                None::<String>
            )
            .execute(&mut *connection)
            .await?;
        }
        ReviewReason::ExcludedActor(value) => {
            sqlx::query_file!(
                "sql/review/insert.sql",
                account,
                kind,
                id,
                original,
                payload,
                shared,
                "actor",
                None::<String>,
                None::<String>,
                None::<String>,
                value.to_string()
            )
            .execute(&mut *connection)
            .await?;
        }
    }
    for (ordinal, value) in review.retained.iter().enumerate() {
        anyhow::ensure!(
            value.key() == key,
            "review alternative crosses record scope"
        );
        let envelope = observation::insert_envelope(connection, value).await?;
        let ordinal = i64::try_from(ordinal)?;
        sqlx::query_file!(
            "sql/review/retained_insert.sql",
            account,
            kind,
            id,
            ordinal,
            envelope
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Restore the exact saved review rather than reclassifying it against changed context.
pub(crate) async fn load(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<Option<ReviewInput>> {
    let account = key.account.to_string();
    let kind = crate::payload::kind(key.id);
    let id = key.id.to_string();
    let Some(row) = sqlx::query_file!("sql/review/select.sql", account, kind, id)
        .fetch_optional(&mut *connection)
        .await?
    else {
        return Ok(None);
    };
    let reason = match (ReviewReasonRow {
        reason: row.reason,
        unknown_reason: row.unknown_reason,
        reference_kind: row.reference_kind,
        reference_id: row.reference_id,
        actor_id: row.actor_id,
    }) {
        ReviewReasonRow {
            reason,
            unknown_reason: None,
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        } if reason == "different" => ReviewReason::BothDifferent,
        ReviewReasonRow {
            reason,
            unknown_reason: None,
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        } if reason == "original" => ReviewReason::MissingOriginal,
        ReviewReasonRow {
            reason,
            unknown_reason: Some(value),
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        } if reason == "ordering" => {
            ReviewReason::OrderingUnknown(observation::read_unknown(&value)?)
        }
        ReviewReasonRow {
            reason,
            unknown_reason: None,
            reference_kind: Some(kind),
            reference_id: Some(id),
            actor_id: None,
        } if reason == "reference" => ReviewReason::MissingReference(RecordKey {
            account: key.account,
            id: crate::payload::record_id(&kind, &id)?,
        }),
        ReviewReasonRow {
            reason,
            unknown_reason: None,
            reference_kind: None,
            reference_id: None,
            actor_id: Some(value),
        } if reason == "actor" => ReviewReason::ExcludedActor(value.parse()?),
        _ => anyhow::bail!("invalid review reason variant"),
    };
    let mut retained = vec![];
    for row in sqlx::query_file!("sql/review/retained.sql", account, kind, id)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(retained.len())?,
            "noncontiguous review alternatives"
        );
        retained.push(observation::load_envelope(connection, key, row.envelope_id).await?);
    }
    Ok(Some(ReviewInput {
        original: original::load_original(connection, key, row.original_id).await?,
        local: crate::payload::load(connection, key, row.local_payload).await?,
        shared: observation::load_observation(connection, key, row.shared_observation).await?,
        reason,
        retained,
    }))
}
