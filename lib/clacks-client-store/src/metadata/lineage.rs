//! Complete-prefix checkpoints and immutable baseline/ancestry interpretations.

use clacks_model::ids::AccountId;
use clacks_sync::{
    account::{Baseline, EvidenceView, InstalledCheckpoint},
    lineage::{BoundaryKind, Coverage, Lineage, TimelineBoundary},
    record::Position,
};
use sqlx::SqliteConnection;

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
    connection: &mut SqliteConnection,
    account: AccountId,
    value: &Coverage,
) -> anyhow::Result<i64> {
    value.validate()?;
    let account = account.to_string();
    let id = sqlx::query_file!("sql/coverage/insert.sql", account)
        .fetch_one(&mut *connection)
        .await?
        .coverage_id;
    for (timeline, revision) in value.entries() {
        let timeline = timeline.to_string();
        let revision = i64::try_from(revision)?;
        sqlx::query_file!("sql/coverage/entry_insert.sql", id, timeline, revision)
            .execute(&mut *connection)
            .await?;
    }
    Ok(id)
}

/// Restore certified prefixes without deriving them from individual record knowledge.
pub(crate) async fn load_coverage(
    connection: &mut SqliteConnection,
    account: AccountId,
    coverage: i64,
) -> anyhow::Result<Coverage> {
    let owner = sqlx::query_file!("sql/coverage/owner.sql", coverage)
        .fetch_one(&mut *connection)
        .await?;
    anyhow::ensure!(
        owner.account == account.to_string(),
        "coverage crosses account"
    );
    let mut entries = Vec::new();
    for row in sqlx::query_file!("sql/coverage/select.sql", coverage)
        .fetch_all(connection)
        .await?
    {
        entries.push((row.timeline_id.parse()?, row.revision.try_into()?));
    }
    Ok(Coverage::from_entries(entries)?)
}

/// Store an immutable position-dependent ancestry snapshot in its existing boundary order.
pub(crate) async fn insert_lineage(
    connection: &mut SqliteConnection,
    value: &Lineage,
) -> anyhow::Result<i64> {
    let account = value.account().to_string();
    let initial = value.initial_timeline().to_string();
    let id = sqlx::query_file!("sql/lineage/insert.sql", account, initial)
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
            "sql/lineage/boundary_insert.sql",
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
    connection: &mut SqliteConnection,
    account: AccountId,
    lineage: i64,
) -> anyhow::Result<Lineage> {
    let account_text = account.to_string();
    let row = sqlx::query_file!("sql/lineage/select.sql", lineage, account_text)
        .fetch_one(&mut *connection)
        .await?;
    let mut result = Lineage::new(account, row.initial_timeline.parse()?);
    for (ordinal, row) in
        sqlx::query_file!("sql/lineage/boundary_select.sql", lineage, account_text)
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
    connection: &mut SqliteConnection,
    value: &Baseline,
) -> anyhow::Result<i64> {
    value.validate()?;
    let account = value.account.to_string();
    let baseline = value.id.to_string();
    if let Some(row) = sqlx::query_file!("sql/baseline/find.sql", account, baseline)
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
        "sql/baseline/insert.sql",
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
    connection: &mut SqliteConnection,
    account: AccountId,
    baseline: i64,
) -> anyhow::Result<Baseline> {
    let account_text = account.to_string();
    let row = sqlx::query_file!("sql/baseline/select.sql", baseline, account_text)
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

/// Store a checkpoint descriptor without making it the current installed receipt.
pub(crate) async fn insert_checkpoint(
    connection: &mut SqliteConnection,
    value: &InstalledCheckpoint,
) -> anyhow::Result<i64> {
    value.validate()?;
    let coverage = insert_coverage(connection, value.account, &value.coverage).await?;
    let account = value.account.to_string();
    let baseline = value.baseline.to_string();
    Ok(
        sqlx::query_file!("sql/checkpoint/insert.sql", account, baseline, coverage)
            .fetch_one(connection)
            .await?
            .checkpoint_id,
    )
}

/// Restore a checkpoint's exact complete-prefix descriptor within its account.
pub(crate) async fn load_checkpoint(
    connection: &mut SqliteConnection,
    account: AccountId,
    checkpoint: i64,
) -> anyhow::Result<InstalledCheckpoint> {
    let account_text = account.to_string();
    let row = sqlx::query_file!("sql/checkpoint/select.sql", checkpoint, account_text)
        .fetch_one(&mut *connection)
        .await?;
    Ok(InstalledCheckpoint {
        account,
        baseline: row.baseline_id.parse()?,
        coverage: load_coverage(connection, account, row.coverage_id).await?,
    })
}

/// Read the actual installed receipt, distinguishing no receipt from zero coverage.
pub(crate) async fn current_checkpoint(
    connection: &mut SqliteConnection,
    account: AccountId,
) -> anyhow::Result<Option<InstalledCheckpoint>> {
    let account_text = account.to_string();
    match sqlx::query_file!("sql/checkpoint/current.sql", account_text)
        .fetch_optional(&mut *connection)
        .await?
    {
        Some(row) => Ok(Some(
            load_checkpoint(connection, account, row.checkpoint_id).await?,
        )),
        None => Ok(None),
    }
}

/// Point current receipt at a checkpoint only within the caller's verified installation transaction.
pub(crate) async fn set_checkpoint(
    connection: &mut SqliteConnection,
    value: &InstalledCheckpoint,
) -> anyhow::Result<()> {
    let checkpoint = insert_checkpoint(connection, value).await?;
    let account = value.account.to_string();
    anyhow::ensure!(
        sqlx::query_file!("sql/checkpoint/current_set.sql", account, checkpoint)
            .execute(connection)
            .await?
            .rows_affected()
            == 1,
        "checkpoint account mismatch"
    );
    Ok(())
}

/// Read actual retained interpretation, returning no context before any genuine ancestry arrives.
pub(crate) async fn interpretation(
    connection: &mut SqliteConnection,
    account: AccountId,
) -> anyhow::Result<Option<Interpretation>> {
    let account_text = account.to_string();
    let Some(row) = sqlx::query_file!("sql/lineage/current.sql", account_text)
        .fetch_optional(&mut *connection)
        .await?
    else {
        return Ok(None);
    };
    let lineage = load_lineage(connection, account, row.lineage_id).await?;
    let mut baselines = Vec::new();
    for row in sqlx::query_file!("sql/baseline/list.sql", account_text)
        .fetch_all(&mut *connection)
        .await?
    {
        baselines.push(load_baseline(connection, account, row.baseline_row).await?);
    }
    Ok(Some(Interpretation { lineage, baselines }))
}

/// Install received ancestry as current without rewriting any retained baseline snapshot.
pub(crate) async fn set_lineage(
    connection: &mut SqliteConnection,
    value: &Lineage,
) -> anyhow::Result<()> {
    let lineage = insert_lineage(connection, value).await?;
    let account = value.account().to_string();
    sqlx::query_file!("sql/lineage/current_set.sql", account, lineage)
        .execute(connection)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{ClientStore, StoreSettings};

    fn id<T: std::str::FromStr>(n: u128) -> T
    where
        T::Err: std::fmt::Debug,
    {
        ulid::Ulid::from(n).to_string().parse().unwrap()
    }

    #[tokio::test]
    async fn stored_ancestry_does_not_apply_later_boundaries_to_earlier_cuts() -> anyhow::Result<()>
    {
        let dir = tempfile::tempdir()?;
        let store = ClientStore::create(
            &dir.path().join("workspace.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let mut ancestry = Lineage::new(id(1), id(2));
        ancestry.add_boundary(TimelineBoundary {
            at: Position {
                timeline: id(4),
                revision: 0,
            },
            includes: Coverage::from(Position {
                timeline: id(2),
                revision: 1,
            }),
            kind: BoundaryKind::ForkOrigin,
        })?;
        ancestry.add_boundary(TimelineBoundary {
            at: Position {
                timeline: id(4),
                revision: 3,
            },
            includes: Coverage::from(Position {
                timeline: id(2),
                revision: 2,
            }),
            kind: BoundaryKind::RecoveredCoverage,
        })?;
        let baseline = Baseline {
            id: id(5),
            account: id(1),
            coverage: Coverage::from(Position {
                timeline: id(4),
                revision: 3,
            }),
            lineage: ancestry.clone(),
            membership_generation: 1,
        };
        let mut tx = store.write().await?;
        let root = insert_baseline(&mut tx, &baseline).await?;
        tx.commit().await?;
        store.close().await?;
        let store = ClientStore::open(
            &dir.path().join("workspace.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let mut tx = store.pool.begin().await?;
        let restored = load_baseline(&mut tx, id(1), root).await?.lineage;
        assert!(
            !restored
                .coverage_at(Position {
                    timeline: id(4),
                    revision: 2
                })?
                .covers(Position {
                    timeline: id(2),
                    revision: 2
                })
        );
        assert!(
            restored
                .coverage_at(Position {
                    timeline: id(4),
                    revision: 3
                })?
                .covers(Position {
                    timeline: id(2),
                    revision: 2
                })
        );
        assert!(load_baseline(&mut tx, id(6), root).await.is_err());
        tx.commit().await?;
        store.close().await?;
        Ok(())
    }
}
