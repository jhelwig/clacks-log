//! Prepare and publish an export on behalf of the active workspace.

use crate::export::{
    ExportFormat, PreparedExport,
    prepare::prepare,
    publish::{ExportDestination, ExportReport, PublishError, publish},
};
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{
        self,
        export::{ExportCounts, ExportInput, ExportScope},
    },
    store::ClientStore,
};
use clacks_model::{
    export::select::ContextResolution,
    ids::{AccountId, QsoId},
    query::Query,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
    time::Instant,
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

/// Applied history inputs retained when the operator opens export setup.
#[derive(Debug)]
pub(crate) struct History {
    /// Saved history view when the operator opened export.
    pub view: SelectedView,
    /// Applied query, which can differ from the current editor draft.
    pub filter: Query,
    /// Explicitly selected contact identities across all history pages.
    pub selected: BTreeSet<QsoId>,
}

/// Three independent contact scopes offered by export setup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Saved contacts in the explicit history selection.
    Selected,
    /// Saved contacts matching the applied history view and query.
    Matching,
    /// All active saved contacts in the workspace.
    All,
}

impl Scope {
    fn store_scope(self, history: &History) -> ExportScope {
        match self {
            Self::Selected => ExportScope::Selected(history.selected.clone()),
            Self::Matching => ExportScope::Matching {
                view: history.view,
                filter: history.filter.clone(),
            },
            Self::All => ExportScope::All,
        }
    }
}

/// Resolved facts and scope counts from the same closed database snapshot.
#[derive(Debug)]
pub(crate) struct Capture {
    /// Working contacts, effective facts and definitions frozen for review.
    pub input: ExportInput,
    /// Contact counts for the choices available at capture time.
    pub counts: ExportCounts,
    /// Liveness of the workspace that supplied the capture.
    pub workspace_stop: CancellationToken,
}

/// One workspace-owned export job.
#[derive(Debug)]
pub(crate) struct Request {
    /// Account whose workspace can accept this request.
    pub account: AccountId,
    /// Local trace identifier shared by the phases of this export.
    pub attempt: u64,
    /// Cancellation signal for the requested phase.
    pub stop: CancellationToken,
    /// Phase the caller requests.
    pub action: Action,
    /// Completion channel for the phase or session error.
    pub reply: oneshot::Sender<Result<Reply, String>>,
}

/// An export phase submitted to the workspace session.
#[derive(Debug)]
pub(crate) enum Action {
    /// Map a contact scope into a reviewable export and resolve contest choices.
    Prepare {
        /// Applied view, query and selection at setup launch.
        history: Arc<History>,
        /// Scope selected by the operator.
        scope: Scope,
        /// Output format selected by the operator.
        format: ExportFormat,
        /// Reviewed snapshot retained while settling choices for this scope.
        capture: Option<Arc<Capture>>,
        /// Operator decisions for ambiguous contest assignments.
        resolutions: BTreeMap<QsoId, ContextResolution>,
    },
    /// Publish a reviewed plan to its chosen destination.
    Publish {
        /// Prepared ADI documents and reported skips.
        plan: Arc<PreparedExport>,
        /// Exact output file or directory and reviewed names.
        destination: ExportDestination,
        /// Liveness of the workspace that produced the plan.
        captured_workspace_stop: CancellationToken,
    },
}

/// UI results distinguish reviewed input from effects on the filesystem.
#[derive(Debug)]
pub(crate) enum Reply {
    /// The prepared plan and the saved input used to produce it.
    Prepared {
        /// Saved input available for a subsequent preparation of this scope.
        capture: Arc<Capture>,
        /// Entries and issues ready to show in export setup.
        plan: Arc<PreparedExport>,
    },
    /// Publication outcome, including completed paths if the attempt failed partway through.
    Published(Result<ExportReport, PublishError>),
}

pub(super) async fn run(
    store: ClientStore,
    request: Request,
    workspace_stop: CancellationToken,
    context: Arc<OnceLock<eframe::egui::Context>>,
) {
    let Request {
        account,
        attempt,
        stop,
        action,
        reply,
    } = request;
    let span = tracing::info_span!("desktop.export", attempt);
    let writer_span = span.clone();
    let result = async {
        anyhow::ensure!(store.account_id() == account, "The export workspace changed. Reopen Export ADI.");
        anyhow::ensure!(!stop.is_cancelled() && !workspace_stop.is_cancelled(), "Export cancelled.");
        match action {
            Action::Prepare { history, scope, format, capture, resolutions } => {
                let started = Instant::now();
                let result = tokio::select! {
                    biased;
                    () = stop.cancelled() => return Ok(None),
                    () = workspace_stop.cancelled() => return Ok(None),
                    result = capture_selection(&store, &history, scope, capture, &workspace_stop) => result,
                };
                let result = match result {
                    Ok(capture) => prepare_selection(capture, format, resolutions).await,
                    Err(error) => Err(error),
                };
                if stop.is_cancelled() || workspace_stop.is_cancelled() { return Ok(None); }
                let outcome = if result.is_ok() { "prepared" } else { "failed" };
                metrics::histogram!("clacks.desktop.export.prepare.duration", "format" => format_label(format), "outcome" => outcome).record(started.elapsed().as_secs_f64());
                if result.is_err() {
                    tracing::warn!(?format, "Export preparation failed");
                }
                result.map(Some)
            }
            Action::Publish { plan, destination, captured_workspace_stop } => {
                anyhow::ensure!(!captured_workspace_stop.is_cancelled(), "The export workspace changed. Reopen Export ADI.");
                let started = Instant::now();
                let format = plan.format;
                let worker_stop = stop.clone();
                let dispatcher = tracing::dispatcher::get_default(Clone::clone);
                let mut worker = tokio::task::spawn_blocking(move || {
                    tracing::dispatcher::with_default(&dispatcher, || writer_span.in_scope(|| publish(&plan, &destination, &worker_stop)))
                });
                let result = tokio::select! {
                    () = workspace_stop.cancelled() => {
                        stop.cancel();
                        worker.await
                    }
                    result = &mut worker => result,
                }.map_err(|_| anyhow::anyhow!("The export writer stopped unexpectedly."))?;
                let (outcome, effects) = match &result {
                    Ok(report) => ("published", report),
                    Err(error) if error.is_cancelled() => ("cancelled", error.report()),
                    Err(error) if error.is_collision() => ("collision", error.report()),
                    Err(error) => ("failed", error.report()),
                };
                metrics::histogram!("clacks.desktop.export.publish.duration", "format" => format_label(format), "outcome" => outcome).record(started.elapsed().as_secs_f64());
                let files = effects.paths.len();
                let entries = effects.entries_written;
                let qsos = effects.qsos_exported;
                if outcome == "failed" {
                    tracing::error!(?format, outcome, files, entries, qsos, "Export publication failed");
                } else {
                    tracing::info!(?format, outcome, files, entries, qsos, "Export publication finished");
                }
                Ok(Some(Reply::Published(result)))
            }
        }
    }.instrument(span).await;
    let result = match result {
        Ok(None) => return,
        Ok(Some(reply)) => Ok(reply),
        Err(error) => Err(format!("{error:#}")),
    };
    let _ = reply.send(result);
    if let Some(context) = context.get() {
        context.request_repaint_of(eframe::egui::ViewportId::ROOT);
    }
}

#[tracing::instrument(skip_all, fields(?scope))]
async fn capture_selection(
    store: &ClientStore,
    history: &History,
    scope: Scope,
    capture: Option<Arc<Capture>>,
    workspace_stop: &CancellationToken,
) -> anyhow::Result<Arc<Capture>> {
    if let Some(capture) = capture {
        anyhow::ensure!(
            !capture.workspace_stop.is_cancelled(),
            "The export workspace changed. Reopen Export ADI."
        );
        Ok(capture)
    } else {
        let mut snapshot = read::snapshot(store).await?;
        let counts = snapshot
            .export_counts(history.view, &history.filter, &history.selected)
            .await?;
        let input = snapshot.export_input(&scope.store_scope(history)).await?;
        snapshot.close().await?;
        Ok(Arc::new(Capture {
            input,
            counts,
            workspace_stop: workspace_stop.clone(),
        }))
    }
}

#[tracing::instrument(skip_all, fields(?format))]
async fn prepare_selection(
    capture: Arc<Capture>,
    format: ExportFormat,
    resolutions: BTreeMap<QsoId, ContextResolution>,
) -> anyhow::Result<Reply> {
    let worker_capture = capture.clone();
    let plan =
        tokio::task::spawn_blocking(move || prepare(&worker_capture.input, format, &resolutions))
            .await??;
    tracing::info!(
        requested = plan.requested_count,
        skipped = plan.skipped.len(),
        files = plan.files.len(),
        "Export prepared for review"
    );
    Ok(Reply::Prepared {
        capture,
        plan: Arc::new(plan),
    })
}

fn format_label(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Ordinary => "ordinary",
        ExportFormat::Pota => "pota",
    }
}
