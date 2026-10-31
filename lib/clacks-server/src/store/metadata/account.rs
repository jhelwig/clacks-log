//! Real account coverage and retained interpretation under the existing server authority.

use super::lineage::{self, Interpretation};
use clacks_model::ids::AccountId;
use clacks_sync::{account::AccountMetadata, record::Position};
use sqlx::PgConnection;

/// Read actual installed coverage rather than reconstructing it from the current head.
pub(crate) async fn load(
    connection: &mut PgConnection,
    account: AccountId,
) -> anyhow::Result<AccountMetadata> {
    let owner = account.to_string();
    let row = sqlx::query_file!("sql/metadata/state_select.sql", owner)
        .fetch_one(&mut *connection)
        .await?;
    let value = AccountMetadata {
        account,
        head: Position {
            timeline: row.timeline_id.parse()?,
            revision: row.revision.try_into()?,
        },
        baseline: row.baseline_id.parse()?,
        installed: lineage::load_coverage(connection, account, row.installed_coverage_id).await?,
        membership_generation: row.membership_generation.try_into()?,
    };
    value.validate()?;
    Ok(value)
}

/// Save native account transitions inside the caller's already guarded transaction.
pub(crate) async fn save(
    connection: &mut PgConnection,
    account: &AccountMetadata,
) -> anyhow::Result<()> {
    account.validate()?;
    let coverage =
        lineage::insert_coverage(connection, account.account, &account.installed).await?;
    let owner = account.account.to_string();
    let timeline = account.head.timeline.to_string();
    let revision = i64::try_from(account.head.revision)?;
    let baseline = account.baseline.to_string();
    let generation = i64::try_from(account.membership_generation)?;
    let result = sqlx::query_file!(
        "sql/metadata/state_update.sql",
        owner,
        timeline,
        revision,
        baseline,
        generation,
        coverage
    )
    .execute(connection)
    .await?;
    anyhow::ensure!(
        result.rows_affected() == 1,
        "account metadata disappeared inside its transaction"
    );
    Ok(())
}

/// Load the complete retained ancestry and baseline definitions for native comparison.
pub(crate) async fn interpretation(
    connection: &mut PgConnection,
    account: AccountId,
) -> anyhow::Result<Interpretation> {
    let owner = account.to_string();
    let state = sqlx::query_file!("sql/metadata/state_select.sql", owner)
        .fetch_one(&mut *connection)
        .await?;
    let lineage = lineage::load_lineage(connection, account, state.current_lineage_id).await?;
    let mut baselines = Vec::new();
    for row in sqlx::query_file!("sql/metadata/baselines.sql", owner)
        .fetch_all(&mut *connection)
        .await?
    {
        baselines.push(lineage::load_baseline(connection, account, row.baseline_row).await?);
    }
    Ok(Interpretation { lineage, baselines })
}
