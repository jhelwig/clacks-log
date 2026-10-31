//! Immutable coupled-effect descriptors independent of a transport receipt.

use clacks_model::ids::AccountId;
use clacks_sync::{
    record::RecordKey,
    service::{VersionReference, operation::CoupledRequirement},
};
use sqlx::SqliteConnection;

/// Preserve an operation's exact members without rewriting an existing operation identity.
pub(crate) async fn remember(
    connection: &mut SqliteConnection,
    account: AccountId,
    unit: &CoupledRequirement,
) -> anyhow::Result<()> {
    let existing = sqlx::query_file!(
        "sql/discovery/members.sql",
        account.to_string(),
        unit.id.to_string()
    )
    .fetch_all(&mut *connection)
    .await?;
    if !existing.is_empty() {
        let mut members = Vec::new();
        for member in existing {
            let key = RecordKey {
                account,
                id: crate::payload::record_id(&member.kind, &member.record_id)?,
            };
            members.push(VersionReference {
                key,
                stamp: super::clock::load_stamp(connection, key, member.stamp_id).await?,
                deleted: member.deleted != 0,
            });
        }
        anyhow::ensure!(
            members == unit.members,
            "coupled operation identity has contradictory effects"
        );
        return Ok(());
    }
    sqlx::query_file!(
        "sql/receipt/unit_insert.sql",
        account.to_string(),
        unit.id.to_string()
    )
    .execute(&mut *connection)
    .await?;
    for (ordinal, member) in unit.members.iter().enumerate() {
        anyhow::ensure!(
            member.key.account == account,
            "coupled effect crosses account"
        );
        let stamp = super::clock::insert_stamp(connection, member.key, &member.stamp).await?;
        sqlx::query_file!(
            "sql/receipt/member_insert.sql",
            account.to_string(),
            unit.id.to_string(),
            i64::try_from(ordinal)?,
            crate::payload::kind(member.key.id),
            member.key.id.to_string(),
            stamp,
            member.deleted
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}
