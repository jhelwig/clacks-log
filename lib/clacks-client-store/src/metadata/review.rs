//! Durable client-owned resolution context returned by native transitions.

use super::client::{observation, original};
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
        ReviewReason::MissingReference(value) => anyhow::ensure!(
            value.account == key.account,
            "review reference crosses account"
        ),
        ReviewReason::Referenced { referrers, .. } => anyhow::ensure!(
            referrers.iter().all(|value| value.account == key.account),
            "review referrer crosses account"
        ),
        _ => {}
    }
    let reason = serde_json::to_string(&review.reason)?;
    sqlx::query_file!(
        "sql/review/insert.sql",
        account,
        kind,
        id,
        original,
        payload,
        shared,
        reason
    )
    .execute(&mut *connection)
    .await?;
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
    let reason = serde_json::from_str::<ReviewReason>(&row.reason)?;
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
