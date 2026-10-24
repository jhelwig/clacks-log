//! Observed server account and membership data, separate from local installed receipt.

use super::lineage;
use clacks_model::ids::AccountId;
use clacks_sync::{
    account::AccountMetadata,
    membership::{ClientMembership, MembershipSnapshot},
    record::Position,
};
use sqlx::SqliteConnection;

/// Store one consistent supplied account/membership view without advancing local receipt.
pub(crate) async fn store(
    connection: &mut SqliteConnection,
    value: &AccountMetadata,
    membership: &MembershipSnapshot,
) -> anyhow::Result<()> {
    membership.validate(value)?;
    let coverage = lineage::insert_coverage(connection, value.account, &value.installed).await?;
    let account = value.account.to_string();
    let timeline = value.head.timeline.to_string();
    let revision = i64::try_from(value.head.revision)?;
    let baseline = value.baseline.to_string();
    let generation = i64::try_from(value.membership_generation)?;
    sqlx::query_file!(
        "sql/account/store.sql",
        account,
        timeline,
        revision,
        baseline,
        coverage,
        generation
    )
    .execute(&mut *connection)
    .await?;
    sqlx::query_file!("sql/account/membership_store.sql", account, generation)
        .execute(&mut *connection)
        .await?;
    sqlx::query_file!("sql/account/member_clear.sql", account)
        .execute(&mut *connection)
        .await?;
    for (ordinal, client) in membership.clients.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let actor = client.actor.to_string();
        let created_timeline = client.created_at.timeline.to_string();
        let created_revision = i64::try_from(client.created_at.revision)?;
        let closed_timeline = client
            .closed_at
            .map(|position| position.timeline.to_string());
        let closed_revision = client
            .closed_at
            .map(|position| i64::try_from(position.revision))
            .transpose()?;
        let checkpoint = match &client.acknowledged {
            Some(checkpoint) => Some(lineage::insert_checkpoint(connection, checkpoint).await?),
            None => None,
        };
        sqlx::query_file!(
            "sql/account/member_insert.sql",
            account,
            actor,
            ordinal,
            created_timeline,
            created_revision,
            closed_timeline,
            closed_revision,
            checkpoint
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Restore observed admission data without interpreting an advertised head as client receipt.
pub(crate) async fn load(
    connection: &mut SqliteConnection,
    account: AccountId,
) -> anyhow::Result<Option<(AccountMetadata, MembershipSnapshot)>> {
    let account_text = account.to_string();
    let Some(row) = sqlx::query_file!("sql/account/select.sql", account_text)
        .fetch_optional(&mut *connection)
        .await?
    else {
        return Ok(None);
    };
    let value = AccountMetadata {
        account,
        head: Position {
            timeline: row.head_timeline.parse()?,
            revision: row.head_revision.try_into()?,
        },
        baseline: row.baseline_id.parse()?,
        installed: lineage::load_coverage(connection, account, row.coverage_id).await?,
        membership_generation: row.membership_generation.try_into()?,
    };
    let membership = sqlx::query_file!("sql/account/membership_select.sql", account_text)
        .fetch_one(&mut *connection)
        .await?;
    let mut clients = Vec::new();
    for row in sqlx::query_file!("sql/account/member_select.sql", account_text)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(clients.len())?,
            "noncontiguous membership snapshot"
        );
        let closed_at = match (row.closed_timeline, row.closed_revision) {
            (None, None) => None,
            (Some(timeline), Some(revision)) => Some(Position {
                timeline: timeline.parse()?,
                revision: revision.try_into()?,
            }),
            _ => anyhow::bail!("partial membership close position"),
        };
        let acknowledged = match row.checkpoint_id {
            Some(checkpoint) => {
                Some(lineage::load_checkpoint(connection, account, checkpoint).await?)
            }
            None => None,
        };
        clients.push(ClientMembership {
            actor: row.actor_id.parse()?,
            created_at: Position {
                timeline: row.created_timeline.parse()?,
                revision: row.created_revision.try_into()?,
            },
            closed_at,
            acknowledged,
        });
    }
    let membership = MembershipSnapshot {
        account,
        generation: membership.generation.try_into()?,
        clients,
    };
    membership.validate(&value)?;
    Ok(Some((value, membership)))
}
