//! Workspace import lifecycle from preparation through local application.

use super::{PrepareRequest, PreparedImport, Report, ReviewSession, RowReadiness, RowReview};
use crate::{
    import::{adi::prepare_source_until, matching::read_candidates},
    synchronization::ExchangeGate,
    telemetry::failure::error_class,
};
use clacks_client_store::{
    read,
    save::import::{ImportRowOutcome, ResolvedImportRow, apply_import_row},
    store::ClientStore,
};
use clacks_model::ids::{QsoId, TagId};
use eframe::egui;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Instant,
};
use tokio::sync::{OwnedRwLockReadGuard, oneshot, watch};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

/// A prepared import that keeps sync paused until review and application end.
struct HeldReview {
    prepared: PreparedImport,
    /// Held through review and application.
    _hold: OwnedRwLockReadGuard<()>,
}

pub(crate) async fn run(
    store: ClientStore,
    gate: ExchangeGate,
    request: PrepareRequest,
    changes: watch::Sender<()>,
    context: Arc<OnceLock<egui::Context>>,
) {
    let PrepareRequest {
        path,
        tag,
        stop,
        review,
        completion,
        progress,
    } = request;
    let run_id = ulid::Ulid::generate();
    let span = tracing::info_span!(
        "desktop.import",
        account_id = %store.account_id(),
        %run_id,
    );
    let result = execute(
        &store,
        &gate,
        (path, tag),
        &stop,
        (review, progress),
        &changes,
        &context,
    )
    .instrument(span.clone())
    .await;
    span.in_scope(|| {
        let outcome = match &result {
            Err(_) => "preparation_failed",
            Ok(report) if report.cancelled => "cancelled",
            Ok(report) if !report.failures.is_empty() => "completed_with_failures",
            Ok(_) => "completed",
        };
        metrics::counter!("clacks_desktop_import_runs_total", "outcome" => outcome).increment(1);
        match &result {
            Ok(report) => tracing::info!(
                event = "desktop.import.settled",
                outcome,
                created = report.created,
                changed = report.changed,
                unchanged = report.unchanged,
                memberships_added = report.memberships,
                assertions_added = report.assertions,
                dropped = report.dropped,
                unprocessed = report.unprocessed,
                failed = report.failures.len(),
                source_truncated = report.terminal.is_some(),
            ),
            Err(error) => tracing::error!(
                event = "desktop.import.preparation_failed",
                error.type = error_class(error),
            ),
        }
    });
    let _ = completion.send(result.map_err(|error| error.to_string()));
    repaint(&context);
}

async fn execute(
    store: &ClientStore,
    gate: &ExchangeGate,
    (path, tag): (PathBuf, TagId),
    stop: &CancellationToken,
    (review, progress): (oneshot::Sender<ReviewSession>, watch::Sender<usize>),
    changes: &watch::Sender<()>,
    context: &OnceLock<egui::Context>,
) -> anyhow::Result<Report> {
    let started = Instant::now();
    let review_snapshot = prepare(store, gate, path, tag, stop).await;
    let outcome = match &review_snapshot {
        Ok(Some(_)) => "ready",
        Ok(None) => "cancelled",
        Err(_) => "failed",
    };
    metrics::histogram!("clacks_desktop_import_prepare_duration_seconds", "outcome" => outcome)
        .record(started.elapsed().as_secs_f64());
    let Some(HeldReview { prepared, _hold }) = review_snapshot? else {
        return Ok(cancelled());
    };

    let (apply, decision) = oneshot::channel();
    if stop.is_cancelled() || review.send(ReviewSession { prepared, apply }).is_err() {
        return Ok(cancelled());
    }
    repaint(context);
    let prepared = tokio::select! {
        biased;
        _ = stop.cancelled() => return Ok(cancelled()),
        result = decision => match result {
            Ok(prepared) => prepared,
            Err(_) => return Ok(cancelled()),
        }
    };
    Ok(apply_rows(store, prepared, stop, changes, &progress, context).await)
}

#[tracing::instrument(skip_all, name = "desktop.import.prepare")]
async fn prepare(
    store: &ClientStore,
    gate: &ExchangeGate,
    path: PathBuf,
    tag: TagId,
    stop: &CancellationToken,
) -> anyhow::Result<Option<HeldReview>> {
    tracing::info!(event = "desktop.import.preparing");
    anyhow::ensure!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                ext.eq_ignore_ascii_case("adi") || ext.eq_ignore_ascii_case("adif")
            }),
        "Choose an ADI file with an .adi or .adif extension"
    );
    let bytes = tokio::select! {
        biased;
        _ = stop.cancelled() => return Ok(None),
        bytes = tokio::fs::read(&path) => bytes?,
    };
    let _hold = match gate.pause(stop).await {
        Ok(hold) => hold,
        Err(_) if stop.is_cancelled() => return Ok(None),
        Err(error) => return Err(error),
    };
    let file = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ADI file".into());
    let prepare_stop = stop.clone();
    let span = tracing::Span::current();
    let subscriber = tracing::dispatcher::get_default(Clone::clone);
    let prepared = tokio::select! {
        biased;
        _ = stop.cancelled() => return Ok(None),
        prepared = async {
            let mut snapshot = read::snapshot(store).await?;
            let settings = snapshot.settings().await?;
            let catalogs = snapshot.import_context().await?;
            let (source, catalogs, identity) = tokio::task::spawn_blocking(move || {
                tracing::dispatcher::with_default(&subscriber, || {
                    span.in_scope(|| {
                        let identity = format!("ADI SHA-256 {}", hex::encode(Sha256::digest(&bytes)));
                        let source = prepare_source_until(&bytes, catalogs.definitions(), || {
                            prepare_stop.is_cancelled()
                        });
                        (source, catalogs, identity)
                    })
                })
            }).await?;
            let candidates = read_candidates(&mut snapshot, store.account_id(), &source.rows).await?;
            snapshot.close().await?;
            let stop = stop.clone();
            tokio::task::spawn_blocking(move || {
                PreparedImport::new(source, file, identity, tag, (settings, catalogs, candidates), &stop)
            }).await?
        } => prepared?,
    };
    let warnings = prepared.header_warnings.len()
        + prepared
            .rows
            .iter()
            .map(|row| row.source.warnings.len())
            .sum::<usize>();
    tracing::info!(
        event = "desktop.import.ready_for_review",
        readable_rows = prepared.rows.len(),
        warnings,
        unsupported_fields = prepared.unsupported.len(),
        source_truncated = prepared.terminal.is_some(),
    );
    Ok(Some(HeldReview { prepared, _hold }))
}

fn cancelled() -> Report {
    Report {
        cancelled: true,
        ..Report::default()
    }
}

#[tracing::instrument(skip_all, name = "desktop.import.apply")]
async fn apply_rows(
    store: &ClientStore,
    prepared: PreparedImport,
    stop: &CancellationToken,
    changes: &watch::Sender<()>,
    progress: &watch::Sender<usize>,
    context: &OnceLock<egui::Context>,
) -> Report {
    let started = Instant::now();
    tracing::info!(
        event = "desktop.import.applying",
        readable_rows = prepared.rows.len()
    );
    let mut stale_failures = 0;
    let mut storage_failures = 0;
    let mut planning_failures = 0;
    let mut report = Report {
        terminal: prepared.terminal.clone(),
        header_warnings: prepared.header_warnings.clone(),
        warnings: prepared
            .rows
            .iter()
            .flat_map(|row| {
                row.source
                    .warnings
                    .iter()
                    .cloned()
                    .map(|warning| (row.source.number, warning))
            })
            .collect(),
        unsupported: prepared.unsupported.clone(),
        ..Report::default()
    };
    // Resolve and validate the complete reviewed set before the first write.
    let mut used = BTreeSet::new();
    let effects: Vec<_> = prepared
        .rows
        .iter()
        .map(|row| (row.source.number, planned_effect(&prepared, row, &mut used)))
        .collect();
    let mut effects = effects.into_iter().enumerate();
    while let Some((row, (number, effect))) = effects.next() {
        if stop.is_cancelled() {
            report.cancelled = true;
            report.unprocessed += 1 + effects.len();
            report.skipped.extend(
                std::iter::once(number)
                    .chain(effects.map(|(_, (number, _))| number))
                    .map(|number| (number, "Cancelled before application")),
            );
            break;
        }
        match effect {
            Ok(Effect::Drop) => {
                report.dropped += 1;
                report.skipped.push((number, "Dropped during review"));
            }
            Ok(Effect::Unprocessed(reason)) => {
                report.unprocessed += 1;
                report.skipped.push((number, reason));
            }
            Ok(Effect::Save(row)) => match apply_import_row(store, *row).await {
                Ok(ImportRowOutcome::Applied {
                    created,
                    qso_changed,
                    memberships_added,
                    assertions_added,
                }) => {
                    report.created += usize::from(created);
                    report.changed += usize::from(qso_changed);
                    report.memberships += memberships_added;
                    report.assertions += assertions_added;
                    report.unchanged += usize::from(!created && !qso_changed);
                    if created || qso_changed || memberships_added != 0 || assertions_added != 0 {
                        changes.send_replace(());
                    }
                }
                Ok(ImportRowOutcome::Stale) => {
                    stale_failures += 1;
                    report
                        .failures
                        .push((number, "The saved QSO changed after preparation.".into()));
                }
                Err(error) => {
                    storage_failures += 1;
                    report.failures.push((number, error.to_string()));
                }
            },
            Err(error) => {
                planning_failures += 1;
                report.failures.push((number, error.to_string()));
            }
        }
        progress.send_replace(row + 1);
        repaint(context);
        tokio::task::yield_now().await;
    }
    let outcome = if report.cancelled {
        "cancelled"
    } else if report.failures.is_empty() {
        "completed"
    } else {
        "completed_with_failures"
    };
    metrics::histogram!("clacks_desktop_import_apply_duration_seconds", "outcome" => outcome)
        .record(started.elapsed().as_secs_f64());
    for (reason, count) in [
        ("planning", planning_failures),
        ("stale", stale_failures),
        ("storage", storage_failures),
    ] {
        if count != 0 {
            metrics::counter!("clacks_desktop_import_row_failures_total", "reason" => reason)
                .increment(count);
        }
    }
    if !report.failures.is_empty() {
        tracing::info!(
            event = "desktop.import.row_failures",
            planning = planning_failures,
            stale = stale_failures,
            storage = storage_failures,
        );
    }
    report
}

enum Effect<'a> {
    Drop,
    Unprocessed(&'static str),
    Save(Box<ResolvedImportRow<'a>>),
}

fn planned_effect<'a>(
    prepared: &'a PreparedImport,
    row: &RowReview,
    used: &mut BTreeSet<QsoId>,
) -> anyhow::Result<Effect<'a>> {
    let (target, qso) = match prepared.row_readiness(row)? {
        RowReadiness::Dropped => return Ok(Effect::Drop),
        RowReadiness::NeedsAssignment => {
            return Ok(Effect::Unprocessed(
                "Source or match choices still required",
            ));
        }
        RowReadiness::NeedsFields => {
            return Ok(Effect::Unprocessed("Field choices still required"));
        }
        RowReadiness::Ready { target, qso } => (target, qso),
    };
    let expected = match target {
        Some(target) => {
            anyhow::ensure!(
                used.insert(target),
                "A saved QSO already received a row in this import"
            );
            Some(
                prepared
                    .index
                    .saved(target)
                    .ok_or_else(|| anyhow::anyhow!("The reviewed target is unavailable"))?
                    .0,
            )
        }
        None => None,
    };
    let projection = &row.projection;
    let assertions = if projection.has_assertions() {
        projection.assertions(
            qso.id(),
            &prepared.source_identity,
            prepared.catalogs.lotw(),
            &prepared.catalogs.projection()?,
        )?
    } else {
        Vec::new()
    };
    Ok(Effect::Save(Box::new(ResolvedImportRow {
        expected,
        qso: *qso,
        assertions,
    })))
}

fn repaint(context: &OnceLock<egui::Context>) {
    if let Some(context) = context.get() {
        context.request_repaint();
    }
}
