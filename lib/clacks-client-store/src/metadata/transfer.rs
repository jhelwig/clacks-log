//! Immutable manifest identities and exact staged/installed transfer progress.

use super::{client::observation, clock, lineage};
use clacks_model::ids::AccountId;
use clacks_sync::{
    account::InstalledCheckpoint,
    lineage::{BoundaryKind, TimelineBoundary},
    record::{Position, RecordKey},
    service::{VersionReference, operation::CoupledRequirement},
    transfer::{InstallBatch, RecordRequirement, WindowManifest, WindowMode, WindowProgress},
};
use sqlx::SqliteConnection;

/// Select the discovery variant without encoding its complete-prefix values as text.
fn mode(value: &WindowMode) -> &'static str {
    match value {
        WindowMode::Full => "full",
        WindowMode::Incremental { .. } => "incremental",
    }
}

/// Find a manifest by decoded source/mode equality, never by an unverified surrogate or hash.
pub(crate) async fn find(
    connection: &mut SqliteConnection,
    source: &InstalledCheckpoint,
    discovery: &WindowMode,
) -> anyhow::Result<Option<i64>> {
    source.validate()?;
    let account = source.account.to_string();
    let baseline = source.baseline.to_string();
    let mode = mode(discovery);
    let mut found = None;
    for candidate in sqlx::query_file!("sql/window/candidates.sql", account, baseline, mode)
        .fetch_all(&mut *connection)
        .await?
    {
        let value = load_manifest(connection, source.account, candidate.window_id).await?;
        if value.source == *source && value.mode == *discovery {
            anyhow::ensure!(found.is_none(), "duplicate immutable window identity");
            found = Some(candidate.window_id);
        }
    }
    Ok(found)
}

/// Store a validated immutable source view or require an identical existing manifest.
pub(crate) async fn remember(
    connection: &mut SqliteConnection,
    incoming: &WindowManifest,
) -> anyhow::Result<i64> {
    let value = WindowManifest::new(
        incoming.account,
        incoming.source.clone(),
        incoming.mode.clone(),
        incoming.records.clone(),
        incoming.boundaries.clone(),
        incoming.baselines.clone(),
        incoming.units.clone(),
    )?;
    if let Some(window) = find(connection, &value.source, &value.mode).await? {
        load_manifest(connection, value.account, window)
            .await?
            .validate_replacement(&value)?;
        return Ok(window);
    }
    let source = lineage::insert_checkpoint(connection, &value.source).await?;
    let after = match &value.mode {
        WindowMode::Full => None,
        WindowMode::Incremental { after } => {
            Some(lineage::insert_coverage(connection, value.account, after).await?)
        }
    };
    let account = value.account.to_string();
    let mode = mode(&value.mode);
    let window = sqlx::query_file!("sql/window/insert.sql", account, source, mode, after)
        .fetch_one(&mut *connection)
        .await?
        .window_id;
    for (ordinal, requirement) in value.records.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let kind = crate::payload::kind(requirement.key.id);
        let id = requirement.key.id.to_string();
        let stamp = clock::insert_stamp(connection, requirement.key, &requirement.stamp).await?;
        sqlx::query_file!(
            "sql/window/requirement_insert.sql",
            window,
            account,
            ordinal,
            kind,
            id,
            stamp,
            requirement.deleted
        )
        .execute(&mut *connection)
        .await?;
    }
    for (ordinal, boundary) in value.boundaries.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let timeline = boundary.at.timeline.to_string();
        let revision = i64::try_from(boundary.at.revision)?;
        let kind = match boundary.kind {
            BoundaryKind::ForkOrigin => "fork",
            BoundaryKind::RecoveredCoverage => "recovered",
        };
        let coverage =
            lineage::insert_coverage(connection, value.account, &boundary.includes).await?;
        sqlx::query_file!(
            "sql/window/boundary_insert.sql",
            window,
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
    for (ordinal, baseline) in value.baselines.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let baseline = lineage::insert_baseline(connection, baseline).await?;
        sqlx::query_file!("sql/window/baseline_insert.sql", window, ordinal, baseline)
            .execute(&mut *connection)
            .await?;
    }
    for (ordinal, unit) in value.units.iter().enumerate() {
        let ordinal = i64::try_from(ordinal)?;
        let operation = unit.id.to_string();
        sqlx::query_file!("sql/window/unit_insert.sql", window, ordinal, operation)
            .execute(&mut *connection)
            .await?;
        for (ordinal, member) in unit.members.iter().enumerate() {
            let ordinal = i64::try_from(ordinal)?;
            let kind = crate::payload::kind(member.key.id);
            let id = member.key.id.to_string();
            let stamp = clock::insert_stamp(connection, member.key, &member.stamp).await?;
            sqlx::query_file!(
                "sql/window/member_insert.sql",
                window,
                operation,
                ordinal,
                account,
                kind,
                id,
                stamp,
                member.deleted
            )
            .execute(&mut *connection)
            .await?;
        }
    }
    sqlx::query_file!("sql/window/progress_create.sql", window)
        .execute(connection)
        .await?;
    Ok(window)
}

/// Restore all immutable requirements and their original interpretation dependencies.
pub(crate) async fn load_manifest(
    connection: &mut SqliteConnection,
    account: AccountId,
    window: i64,
) -> anyhow::Result<WindowManifest> {
    let account_text = account.to_string();
    let row = sqlx::query_file!("sql/window/select.sql", window, account_text)
        .fetch_one(&mut *connection)
        .await?;
    let source = lineage::load_checkpoint(connection, account, row.source_checkpoint).await?;
    let mode = match (row.mode.as_str(), row.after_coverage) {
        ("full", None) => WindowMode::Full,
        ("incremental", Some(coverage)) => WindowMode::Incremental {
            after: lineage::load_coverage(connection, account, coverage).await?,
        },
        _ => anyhow::bail!("invalid window discovery variant"),
    };
    let mut records = Vec::new();
    for row in sqlx::query_file!("sql/window/requirement_select.sql", window, account_text)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(records.len())? && matches!(row.deleted, 0 | 1),
            "invalid ordered window requirement"
        );
        let key = RecordKey {
            account,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        records.push(RecordRequirement {
            key,
            stamp: clock::load_stamp(connection, key, row.stamp_id).await?,
            deleted: row.deleted != 0,
        });
    }
    let mut boundaries = Vec::new();
    for row in sqlx::query_file!("sql/window/boundary_select.sql", window, account_text)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(boundaries.len())?,
            "noncontiguous window boundaries"
        );
        boundaries.push(TimelineBoundary {
            at: Position {
                timeline: row.timeline_id.parse()?,
                revision: row.revision.try_into()?,
            },
            includes: lineage::load_coverage(connection, account, row.coverage_id).await?,
            kind: match row.kind.as_str() {
                "fork" => BoundaryKind::ForkOrigin,
                "recovered" => BoundaryKind::RecoveredCoverage,
                _ => anyhow::bail!("unknown window boundary kind"),
            },
        });
    }
    let mut baselines = Vec::new();
    for row in sqlx::query_file!("sql/window/baseline_select.sql", window)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(baselines.len())?,
            "noncontiguous window baselines"
        );
        baselines.push(lineage::load_baseline(connection, account, row.baseline_row).await?);
    }
    let mut units = Vec::new();
    for row in sqlx::query_file!("sql/window/unit_select.sql", window)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(units.len())?,
            "noncontiguous atomic units"
        );
        let mut members = Vec::new();
        for member in sqlx::query_file!("sql/window/member_select.sql", window, row.operation_id)
            .fetch_all(&mut *connection)
            .await?
        {
            anyhow::ensure!(
                member.ordinal == i64::try_from(members.len())?
                    && member.account == account_text
                    && matches!(member.deleted, 0 | 1),
                "invalid atomic member scope or sequence"
            );
            let key = RecordKey {
                account,
                id: crate::payload::record_id(&member.kind, &member.record_id)?,
            };
            members.push(VersionReference {
                key,
                stamp: clock::load_stamp(connection, key, member.stamp_id).await?,
                deleted: member.deleted != 0,
            });
        }
        units.push(CoupledRequirement {
            id: row.operation_id.parse()?,
            members,
        });
    }
    WindowManifest::new(account, source, mode, records, boundaries, baselines, units)
}

/// Read received progress without treating staged envelopes as installed requirements.
pub(crate) async fn progress(
    connection: &mut SqliteConnection,
    account: AccountId,
    window: i64,
) -> anyhow::Result<WindowProgress> {
    let manifest = load_manifest(connection, account, window).await?;
    let row = sqlx::query_file!("sql/window/progress_select.sql", window)
        .fetch_one(&mut *connection)
        .await?;
    let mut installed = std::collections::BTreeSet::new();
    for key in sqlx::query_file!("sql/window/installed_select.sql", window)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            key.account == account.to_string(),
            "installed key crosses account"
        );
        anyhow::ensure!(
            installed.insert(RecordKey {
                account,
                id: crate::payload::record_id(&key.kind, &key.record_id)?
            }),
            "duplicate installed key"
        );
    }
    let mut staged = Vec::new();
    for row in sqlx::query_file!("sql/window/staged_select.sql", window)
        .fetch_all(&mut *connection)
        .await?
    {
        anyhow::ensure!(
            row.ordinal == i64::try_from(staged.len())? && row.account == account.to_string(),
            "invalid staged input scope or sequence"
        );
        let key = RecordKey {
            account,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        staged.push(observation::load_envelope(connection, key, row.envelope_id).await?);
    }
    let mut completed_units = std::collections::BTreeSet::new();
    for row in sqlx::query_file!("sql/window/completed_select.sql", window)
        .fetch_all(connection)
        .await?
    {
        anyhow::ensure!(
            completed_units.insert(row.operation_id.parse()?),
            "duplicate completed atomic unit"
        );
    }
    Ok(WindowProgress {
        source: manifest.source,
        mode: manifest.mode,
        generation: row.generation.try_into()?,
        installed,
        staged,
        completed_units,
    })
}

/// Persist core-computed batch progress under its actual prior generation inside one transaction.
pub(crate) async fn store_batch(
    connection: &mut SqliteConnection,
    account: AccountId,
    window: i64,
    batch: &InstallBatch,
) -> anyhow::Result<()> {
    let manifest = load_manifest(connection, account, window).await?;
    anyhow::ensure!(
        manifest.source == batch.source && manifest.mode == batch.mode,
        "batch belongs to another immutable window"
    );
    let expected = i64::try_from(batch.expected_generation)?;
    let next = i64::try_from(batch.next_generation)?;
    anyhow::ensure!(
        next == expected
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("window progress exhausted"))?,
        "window generation must advance exactly once"
    );
    anyhow::ensure!(
        sqlx::query_file!("sql/window/progress_update.sql", next, window, expected)
            .execute(&mut *connection)
            .await?
            .rows_affected()
            == 1,
        "stale window progress"
    );
    let account_text = account.to_string();
    for key in &batch.installed_keys {
        anyhow::ensure!(key.account == account, "installed key crosses account");
        let kind = crate::payload::kind(key.id);
        let id = key.id.to_string();
        sqlx::query_file!(
            "sql/window/installed_insert.sql",
            window,
            account_text,
            kind,
            id
        )
        .execute(&mut *connection)
        .await?;
    }
    sqlx::query_file!("sql/window/staged_clear.sql", window)
        .execute(&mut *connection)
        .await?;
    for (ordinal, value) in batch.staged.iter().enumerate() {
        anyhow::ensure!(
            value.key.account == account,
            "staged envelope crosses account"
        );
        let ordinal = i64::try_from(ordinal)?;
        let kind = crate::payload::kind(value.key.id);
        let id = value.key.id.to_string();
        let envelope = observation::insert_envelope(connection, value).await?;
        sqlx::query_file!(
            "sql/window/staged_insert.sql",
            window,
            ordinal,
            account_text,
            kind,
            id,
            envelope
        )
        .execute(&mut *connection)
        .await?;
    }
    sqlx::query_file!("sql/window/completed_clear.sql", window)
        .execute(&mut *connection)
        .await?;
    for operation in &batch.completed_units {
        let operation = operation.to_string();
        sqlx::query_file!("sql/window/completed_insert.sql", window, operation)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}
