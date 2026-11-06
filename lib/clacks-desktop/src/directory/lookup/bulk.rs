//! Selection-owned bulk lookup on the ordinary workspace directory runtime.
pub(crate) mod report;

use super::{
    LookupClient, LookupState,
    failure::{AdmissionBlock, LookupFailure, LookupFailureKind},
};
use clacks_client_store::{
    directory::snapshots::{self, SnapshotChange, SnapshotIneligible, SnapshotOutcome},
    read,
};
use clacks_model::{
    directory::values::LookupKey,
    ids::{AccountId, QsoId},
};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use report::{
    BulkContact, BulkCounts, BulkDetail, BulkDetailOutcome, BulkFailure, BulkReport, BulkStop,
    BulkUpdate, UnprocessedReason,
};
use std::{collections::BTreeSet, sync::Arc};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, instrument::WithSubscriber};

/// One UI consumer's cancellation/progress, independent of shared provider lifetime.
#[derive(Debug)]
pub(crate) struct BulkLookupHandle {
    /// Cooperatively stops this consumer at a safe application boundary.
    pub cancel: CancellationToken,
    /// Latest compact state and eventual complete terminal report.
    pub updates: watch::Receiver<BulkUpdate>,
}

/// A captured contact may have disappeared before target preparation.
struct BulkTarget {
    /// Captured identifying values for reporting.
    contact: BulkContact,
    /// Requested original key, absent for an unavailable parent.
    key: Option<LookupKey>,
}

/// Correlation and measurement boundary captured at actual run admission.
struct RunContext {
    /// Owning local account for the operation span.
    account: AccountId,
    /// Fresh session-only run identity, unrelated to provider lookup identities.
    run_id: ulid::Ulid,
    /// Admission instant, including time waiting for the executor to poll the task.
    started: std::time::Instant,
}

impl LookupClient {
    /// Admit a selected-contact consumer on the existing tracked workspace runtime.
    pub fn start_bulk(
        &self,
        ids: BTreeSet<QsoId>,
        changes: watch::Sender<()>,
    ) -> anyhow::Result<BulkLookupHandle> {
        anyhow::ensure!(
            !ids.is_empty(),
            "Select contacts before looking up directory information"
        );
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Directory lookup state is unavailable"))?;
        anyhow::ensure!(!state.closed, "The workspace is closing");
        let total = ids.len() as u64;
        let (updates, receiver) = watch::channel(BulkUpdate::Preparing { total });
        let cancel = CancellationToken::new();
        let client = self.clone();
        let task_cancel = cancel.clone();
        let context = RunContext {
            account: self.inner.store.account_id(),
            run_id: ulid::Ulid::generate(),
            started: std::time::Instant::now(),
        };
        self.inner.tasks.spawn_on(
            async move {
                run(client, ids, changes, updates, task_cancel, context, total).await;
            }
            .in_current_span()
            .with_current_subscriber(),
            &self.inner.executor,
        );
        drop(state);
        Ok(BulkLookupHandle {
            cancel,
            updates: receiver,
        })
    }
}

/// Resolve one stable selected set before provider access, using only saved working values.
async fn capture(client: &LookupClient, ids: &BTreeSet<QsoId>) -> anyhow::Result<Vec<BulkTarget>> {
    let mut snapshot = read::snapshot(&client.inner.store).await?;
    let mut targets = Vec::with_capacity(ids.len());
    for id in ids {
        let row = snapshot
            .record(RecordKey {
                account: client.inner.store.account_id(),
                id: RecordId::Qso(*id),
            })
            .await?;
        let target = match row.as_ref().and_then(|row| row.working_state()) {
            Some(RecordState::Live(RecordValue::Qso(qso))) => BulkTarget {
                contact: BulkContact {
                    id: *id,
                    callsign: Some(qso.contacted_station().callsign.clone()),
                    start: Some(qso.start()),
                },
                key: Some(LookupKey::from_recorded(&qso.contacted_station().callsign)),
            },
            _ => BulkTarget {
                contact: unknown(*id),
                key: None,
            },
        };
        targets.push(target);
    }
    snapshot.close().await?;
    Ok(targets)
}

/// Keep an unreadable selected identity without inventing its former on-air facts.
fn unknown(id: QsoId) -> BulkContact {
    BulkContact {
        id,
        callsign: None,
        start: None,
    }
}

/// Add one skipped member once, including its truthful metric outcome.
fn unprocessed(report: &mut BulkReport, contact: BulkContact, reason: UnprocessedReason) {
    report.counts.unprocessed += 1;
    report.details.push(BulkDetail {
        contact,
        outcome: BulkDetailOutcome::Unprocessed { reason },
    });
    metrics::counter!("clacks_directory_bulk_contacts_total", "outcome" => "unprocessed")
        .increment(1);
}

/// Settle one failed operation without disguising local storage as provider success.
fn failed(report: &mut BulkReport, contact: BulkContact, failure: BulkFailure) {
    report.counts.failed += 1;
    report.details.push(BulkDetail {
        contact,
        outcome: BulkDetailOutcome::Failed { failure },
    });
    metrics::counter!("clacks_directory_bulk_contacts_total", "outcome" => "failed").increment(1);
}

/// Publish the current compact state and wake the owning native view.
fn publish(client: &LookupClient, updates: &watch::Sender<BulkUpdate>, update: BulkUpdate) {
    updates.send_replace(update);
    if let Some(context) = client.inner.context.get() {
        context.request_repaint();
    }
}

/// Use actual current admission state after an unsuccessful ordinary request.
fn blocked(client: &LookupClient) -> Option<BulkFailure> {
    client.admission_block().map(|reason| match reason {
        AdmissionBlock::Authentication => BulkFailure::Authentication,
        reason @ AdmissionBlock::RateLimited { .. } => BulkFailure::Lookup {
            failure: LookupFailure::new(LookupFailureKind::RateLimited, reason.to_string()),
        },
        other => BulkFailure::Configuration {
            message: other.to_string(),
        },
    })
}

/// Run one admitted selection; cancellation never races a started database write.
#[tracing::instrument(skip_all, name="desktop.directory.bulk", fields(account_id=%context.account,run_id=%context.run_id,total))]
async fn run(
    client: LookupClient,
    ids: BTreeSet<QsoId>,
    changes: watch::Sender<()>,
    updates: watch::Sender<BulkUpdate>,
    cancel: CancellationToken,
    context: RunContext,
    total: u64,
) {
    let mut report = BulkReport {
        total,
        counts: BulkCounts::default(),
        details: Vec::new(),
        stop: BulkStop::Completed,
    };
    let captured = tokio::select! {
        biased;
        _ = cancel.cancelled() => None,
        _ = client.inner.stop.cancelled() => None,
        value = capture(&client, &ids) => Some(value),
    };
    match captured {
        None => {
            report.stop = BulkStop::Cancelled;
            for id in ids {
                unprocessed(&mut report, unknown(id), UnprocessedReason::Cancelled);
            }
        }
        Some(Err(error)) => {
            let message = error.to_string();
            tracing::error!(error.type=crate::telemetry::failure::error_class(&error), "Bulk lookup preparation failed");
            report.stop = BulkStop::PreparationFailed {
                message: message.clone(),
            };
            let failure = BulkFailure::Storage { message };
            for id in ids {
                unprocessed(
                    &mut report,
                    unknown(id),
                    UnprocessedReason::Stopped {
                        failure: failure.clone(),
                    },
                );
            }
        }
        Some(Ok(targets)) => {
            publish(
                &client,
                &updates,
                BulkUpdate::Running {
                    total,
                    counts: report.counts.clone(),
                },
            );
            let mut remaining = targets.into_iter();
            for target in remaining.by_ref() {
                let stopped = cancel.is_cancelled() || client.inner.stop.is_cancelled();
                if stopped {
                    report.stop = BulkStop::Cancelled;
                    unprocessed(&mut report, target.contact, UnprocessedReason::Cancelled);
                    break;
                }
                let Some(key) = target.key else {
                    unprocessed(
                        &mut report,
                        target.contact,
                        UnprocessedReason::ParentUnavailable,
                    );
                    continue;
                };
                let handle = client.request(key);
                let outcome = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => None,
                    _ = client.inner.stop.cancelled() => None,
                    outcome = handle.wait() => Some(outcome),
                };
                let Some(outcome) = outcome else {
                    report.stop = BulkStop::Cancelled;
                    unprocessed(&mut report, target.contact, UnprocessedReason::Cancelled);
                    break;
                };
                if cancel.is_cancelled() || client.inner.stop.is_cancelled() {
                    report.stop = BulkStop::Cancelled;
                    unprocessed(&mut report, target.contact, UnprocessedReason::Cancelled);
                    break;
                }
                let mut stop = None;
                match outcome.as_ref() {
                    LookupState::Found(snapshot) => {
                        match snapshots::apply(
                            &client.inner.store,
                            target.contact.id,
                            snapshot.as_ref().clone(),
                        )
                        .await
                        {
                            Ok(SnapshotOutcome::Applied { change }) => {
                                changes.send_replace(());
                                if change == SnapshotChange::MetadataOnly {
                                    report.counts.unchanged += 1;
                                    metrics::counter!("clacks_directory_bulk_contacts_total", "outcome" => "unchanged").increment(1);
                                } else {
                                    report.counts.new_updated += 1;
                                    metrics::counter!("clacks_directory_bulk_contacts_total", "outcome" => "new_updated").increment(1);
                                }
                            }
                            Ok(SnapshotOutcome::Unchanged) => {
                                report.counts.unchanged += 1;
                                metrics::counter!("clacks_directory_bulk_contacts_total", "outcome" => "unchanged").increment(1);
                            }
                            Ok(SnapshotOutcome::Ineligible { reason }) => unprocessed(
                                &mut report,
                                target.contact,
                                match reason {
                                    SnapshotIneligible::ParentUnavailable => {
                                        UnprocessedReason::ParentUnavailable
                                    }
                                    SnapshotIneligible::LookupKeyChanged => {
                                        UnprocessedReason::LookupKeyChanged
                                    }
                                },
                            ),
                            Err(error) => {
                                tracing::error!(qso_id=%target.contact.id,error.type=crate::telemetry::failure::error_class(&error), "Bulk snapshot application failed");
                                let failure = BulkFailure::Storage {
                                    message: error.to_string(),
                                };
                                stop = Some(failure.clone());
                                failed(&mut report, target.contact, failure);
                            }
                        }
                    }
                    LookupState::NotFound => {
                        report.counts.not_found += 1;
                        metrics::counter!("clacks_directory_bulk_contacts_total", "outcome" => "not_found").increment(1);
                    }
                    LookupState::Failed(failure) => {
                        if failure.kind == LookupFailureKind::RateLimited
                            && failure
                                .origin
                                .as_ref()
                                .is_some_and(|origin| client.is_current_origin(origin))
                        {
                            stop = blocked(&client).or_else(|| {
                                Some(BulkFailure::Lookup {
                                    failure: failure.clone(),
                                })
                            });
                        }
                        stop = stop.or_else(|| blocked(&client));
                        failed(
                            &mut report,
                            target.contact,
                            BulkFailure::Lookup {
                                failure: failure.clone(),
                            },
                        );
                    }
                    LookupState::PausedAuthentication => {
                        stop = blocked(&client);
                        failed(&mut report, target.contact, BulkFailure::Authentication);
                    }
                    LookupState::Cleared | LookupState::Pending => {
                        let failure = blocked(&client).unwrap_or(BulkFailure::Configuration {
                            message: "Directory lookup is unavailable".into(),
                        });
                        stop = Some(failure.clone());
                        failed(&mut report, target.contact, failure);
                    }
                }
                publish(
                    &client,
                    &updates,
                    BulkUpdate::Running {
                        total,
                        counts: report.counts.clone(),
                    },
                );
                if let Some(reason) = stop {
                    report.stop = BulkStop::Stopped { reason };
                    break;
                }
            }
            for target in remaining {
                let reason = match &report.stop {
                    BulkStop::Cancelled => UnprocessedReason::Cancelled,
                    BulkStop::Stopped { reason } => UnprocessedReason::Stopped {
                        failure: reason.clone(),
                    },
                    _ => UnprocessedReason::ParentUnavailable,
                };
                unprocessed(&mut report, target.contact, reason);
            }
        }
    }
    let outcome = match &report.stop {
        BulkStop::Completed => "completed",
        BulkStop::Cancelled => "cancelled",
        BulkStop::Stopped { .. } => "stopped",
        BulkStop::PreparationFailed { .. } => "preparation_failed",
    };
    metrics::counter!("clacks_directory_bulk_runs_total", "outcome" => outcome).increment(1);
    metrics::histogram!("clacks_directory_bulk_duration_seconds", "outcome" => outcome)
        .record(context.started.elapsed().as_secs_f64());
    tracing::info!(
        outcome,
        new_updated = report.counts.new_updated,
        unchanged = report.counts.unchanged,
        not_found = report.counts.not_found,
        failed = report.counts.failed,
        unprocessed = report.counts.unprocessed,
        "Bulk directory lookup settled"
    );
    publish(&client, &updates, BulkUpdate::Finished(Arc::new(report)));
}
