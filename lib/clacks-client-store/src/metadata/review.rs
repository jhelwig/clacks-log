//! Durable client-owned resolution context returned by native transitions.

use super::client::{observation, original};
use crate::rows::review::ReviewReasonFields;
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
    let fields = match &review.reason {
        ReviewReason::BothDifferent => ReviewReasonFields {
            reason: "different".into(),
            unknown_reason: None,
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        },
        ReviewReason::MissingOriginal => ReviewReasonFields {
            reason: "original".into(),
            unknown_reason: None,
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        },
        ReviewReason::OrderingUnknown(value) => ReviewReasonFields {
            reason: "ordering".into(),
            unknown_reason: Some(observation::unknown(*value).into()),
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        },
        ReviewReason::MissingReference(value) => {
            anyhow::ensure!(
                value.account == key.account,
                "review reference crosses account"
            );
            crate::payload::identity(connection, *value).await?;
            ReviewReasonFields {
                reason: "reference".into(),
                unknown_reason: None,
                reference_kind: Some(crate::payload::kind(value.id).into()),
                reference_id: Some(value.id.to_string()),
                actor_id: None,
            }
        }
        ReviewReason::ExcludedActor(value) => ReviewReasonFields {
            reason: "actor".into(),
            unknown_reason: None,
            reference_kind: None,
            reference_id: None,
            actor_id: Some(value.to_string()),
        },
    };
    sqlx::query_file!(
        "sql/review/insert.sql",
        account,
        kind,
        id,
        original,
        payload,
        shared,
        fields.reason,
        fields.unknown_reason,
        fields.reference_kind,
        fields.reference_id,
        fields.actor_id
    )
    .execute(&mut *connection)
    .await?;
    for (ordinal, value) in review.retained.iter().enumerate() {
        anyhow::ensure!(value.key == key, "review alternative crosses record scope");
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
    let reason = match (ReviewReasonFields {
        reason: row.reason,
        unknown_reason: row.unknown_reason,
        reference_kind: row.reference_kind,
        reference_id: row.reference_id,
        actor_id: row.actor_id,
    }) {
        ReviewReasonFields {
            reason,
            unknown_reason: None,
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        } if reason == "different" => ReviewReason::BothDifferent,
        ReviewReasonFields {
            reason,
            unknown_reason: None,
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        } if reason == "original" => ReviewReason::MissingOriginal,
        ReviewReasonFields {
            reason,
            unknown_reason: Some(value),
            reference_kind: None,
            reference_id: None,
            actor_id: None,
        } if reason == "ordering" => {
            ReviewReason::OrderingUnknown(observation::read_unknown(&value)?)
        }
        ReviewReasonFields {
            reason,
            unknown_reason: None,
            reference_kind: Some(kind),
            reference_id: Some(id),
            actor_id: None,
        } if reason == "reference" => ReviewReason::MissingReference(RecordKey {
            account: key.account,
            id: crate::payload::record_id(&kind, &id)?,
        }),
        ReviewReasonFields {
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
