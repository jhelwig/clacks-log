//! Complete-prefix checkpoints and immutable baseline/ancestry interpretations.

use clacks_model::ids::AccountId;
use clacks_sync::{
    account::{Baseline, EvidenceView, InstalledCheckpoint},
    lineage::{BoundaryKind, Coverage, Lineage, TimelineBoundary},
    record::Position,
};
use sqlx::PgConnection;

/// Owned backing data for an interpretation borrowed by a pure core transition.
pub(crate) struct Interpretation {
    /// Actual retained account ancestry.
    pub(crate) lineage: Lineage,
    /// Installed immutable baseline meanings.
    pub(crate) baselines: Vec<Baseline>,
}

impl Interpretation {
    /// Borrow the complete retained interpretation without extending its lifetime.
    pub(crate) fn view(&self) -> EvidenceView<'_> {
        EvidenceView {
            lineage: &self.lineage,
            baselines: &self.baselines,
        }
    }
}

/// Store explicit complete prefixes, including meaningful zero coordinates.
pub(crate) async fn insert_coverage(
    connection: &mut PgConnection,
    account: AccountId,
    value: &Coverage,
) -> anyhow::Result<i64> {
    let account = account.to_string();
    value.validate()?;
    let body = serde_json::to_string(value)?;
    let id = sqlx::query_file!("sql/persistence/coverage/insert.sql", account, body)
        .fetch_one(&mut *connection)
        .await?
        .coverage_id;
    Ok(id)
}

/// Restore certified prefixes without deriving them from individual record knowledge.
pub(crate) async fn load_coverage(
    connection: &mut PgConnection,
    account: AccountId,
    coverage: i64,
) -> anyhow::Result<Coverage> {
    let owner = sqlx::query_file!(
        "sql/persistence/coverage/owner.sql",
        coverage,
        account.to_string()
    )
    .fetch_one(&mut *connection)
    .await?;
    Ok(serde_json::from_str(&owner.body)?)
}

/// Store an immutable position-dependent ancestry snapshot in its existing boundary order.
pub(crate) async fn insert_lineage(
    connection: &mut PgConnection,
    value: &Lineage,
) -> anyhow::Result<i64> {
    let account = value.account().to_string();
    let initial = value.initial_timeline().to_string();
    let id = sqlx::query_file!("sql/persistence/lineage/insert.sql", account, initial)
        .fetch_one(&mut *connection)
        .await?
        .lineage_id;
    for (ordinal, boundary) in value.boundaries().iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let timeline = boundary.at.timeline.to_string();
        let revision = i64::try_from(boundary.at.revision)?;
        let kind = match boundary.kind {
            BoundaryKind::ForkOrigin => "fork",
            BoundaryKind::RecoveredCoverage => "recovered",
        };
        let coverage = insert_coverage(connection, value.account(), &boundary.includes).await?;
        sqlx::query_file!(
            "sql/persistence/lineage/boundary_insert.sql",
            id,
            account,
            ordinal,
            timeline,
            revision,
            kind,
            coverage
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(id)
}

/// Restore ancestry through native boundary validation, preserving when each claim became available.
pub(crate) async fn load_lineage(
    connection: &mut PgConnection,
    account: AccountId,
    lineage: i64,
) -> anyhow::Result<Lineage> {
    let account_text = account.to_string();
    let row = sqlx::query_file!("sql/persistence/lineage/select.sql", lineage, account_text)
        .fetch_one(&mut *connection)
        .await?;
    let mut result = Lineage::new(account, row.initial_timeline.parse()?);
    for (ordinal, row) in sqlx::query_file!(
        "sql/persistence/lineage/boundary_select.sql",
        lineage,
        account_text
    )
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .enumerate()
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(ordinal)?,
            "noncontiguous timeline boundaries"
        );
        result.add_boundary(TimelineBoundary {
            at: Position {
                timeline: row.timeline_id.parse()?,
                revision: row.revision.try_into()?,
            },
            includes: load_coverage(connection, account, row.coverage_id).await?,
            kind: match row.kind.as_str() {
                "fork" => BoundaryKind::ForkOrigin,
                "recovered" => BoundaryKind::RecoveredCoverage,
                _ => anyhow::bail!("unknown timeline boundary kind"),
            },
        })?;
    }
    Ok(result)
}

/// Insert an immutable baseline, accepting replay only when every retained meaning agrees.
pub(crate) async fn insert_baseline(
    connection: &mut PgConnection,
    value: &Baseline,
) -> anyhow::Result<i64> {
    value.validate()?;
    let account = value.account.to_string();
    let baseline = value.id.to_string();
    if let Some(row) = sqlx::query_file!("sql/persistence/baseline/find.sql", account, baseline)
        .fetch_optional(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            load_baseline(connection, value.account, row.baseline_row).await? == *value,
            "baseline {baseline} has incompatible retained meanings"
        );
        return Ok(row.baseline_row);
    }
    let coverage = insert_coverage(connection, value.account, &value.coverage).await?;
    let lineage = insert_lineage(connection, &value.lineage).await?;
    let generation = i64::try_from(value.membership_generation)?;
    Ok(sqlx::query_file!(
        "sql/persistence/baseline/insert.sql",
        account,
        baseline,
        coverage,
        lineage,
        generation
    )
    .fetch_one(connection)
    .await?
    .baseline_row)
}

/// Restore one baseline's complete meaning rather than assigning the current interpretation to an old ID.
pub(crate) async fn load_baseline(
    connection: &mut PgConnection,
    account: AccountId,
    baseline: i64,
) -> anyhow::Result<Baseline> {
    let account_text = account.to_string();
    let row = sqlx::query_file!(
        "sql/persistence/baseline/select.sql",
        baseline,
        account_text
    )
    .fetch_one(&mut *connection)
    .await?;
    let value = Baseline {
        id: row.baseline_id.parse()?,
        account,
        coverage: load_coverage(connection, account, row.coverage_id).await?,
        lineage: load_lineage(connection, account, row.lineage_id).await?,
        membership_generation: row.membership_generation.try_into()?,
    };
    value.validate()?;
    Ok(value)
}

/// Restore a checkpoint's exact complete-prefix descriptor within its account.
pub(crate) async fn load_checkpoint(
    connection: &mut PgConnection,
    account: AccountId,
    checkpoint: i64,
) -> anyhow::Result<InstalledCheckpoint> {
    let account_text = account.to_string();
    let row = sqlx::query_file!(
        "sql/persistence/checkpoint/select.sql",
        checkpoint,
        account_text
    )
    .fetch_one(&mut *connection)
    .await?;
    Ok(InstalledCheckpoint {
        account,
        baseline: row.baseline_id.parse()?,
        coverage: load_coverage(connection, account, row.coverage_id).await?,
    })
}

/// Retain the exact acknowledged checkpoint, separate from current account coverage.
pub(crate) async fn insert_checkpoint(
    connection: &mut PgConnection,
    value: &InstalledCheckpoint,
) -> anyhow::Result<i64> {
    value.validate()?;
    let coverage = insert_coverage(connection, value.account, &value.coverage).await?;
    Ok(sqlx::query_file!(
        "sql/persistence/checkpoint/insert.sql",
        value.account.to_string(),
        value.baseline.to_string(),
        coverage
    )
    .fetch_one(connection)
    .await?
    .checkpoint_id)
}
