//! Receive-first synchronization cycles over the actual SQLite receipt adapter.

use super::{ConnectionContext, ServerFailure, Socket, pending, receive, send};
use clacks_client_store::{
    identity::EnrollmentBinding, settings::SettingsSnapshot, store::ClientStore, synchronization,
    transfer,
};
use clacks_sync::protocol::messages::{self, ClientMessage, ServerMessage, TransferMessage};
use std::sync::Arc;
use tokio::sync::watch;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Consume wakeups while waiting for an ordered reply; wakeups never establish coverage.
pub(super) async fn response(
    socket: &mut Socket,
    store: &ClientStore,
    dirty: &mut bool,
    connection: &ConnectionContext,
) -> anyhow::Result<ServerMessage> {
    loop {
        match receive(socket, connection).await? {
            ServerMessage::Changed => *dirty = true,
            ServerMessage::Failure { kind, message } => {
                return Err(ServerFailure { kind, message }.into());
            }
            ServerMessage::Review { input } => {
                synchronization::receive_review(store, &input).await?;
            }
            message => return Ok(message),
        }
    }
}

/// Negotiate, repair retained accepted state if needed, then alternate complete inbound and pending phases.
#[tracing::instrument(skip_all, name = "desktop.sync.exchange_session", fields(account_id = %binding.account, actor_id = %binding.actor))]
pub(super) async fn run(
    socket: &mut Socket,
    store: &ClientStore,
    binding: &EnrollmentBinding,
    changes: &mut watch::Receiver<()>,
    updates: &watch::Sender<Option<Arc<SettingsSnapshot>>>,
    connection: &ConnectionContext,
) -> anyhow::Result<()> {
    let stop = &connection.stop;
    let negotiation_guard = connection.gate.exchange(stop).await?;
    let negotiation = tracing::info_span!(parent: None, "desktop.sync.negotiate", account_id=%binding.account, actor_id=%binding.actor);
    negotiation.add_link(connection.trace.clone());
    negotiation.in_scope(|| tracing::info!(event="sync.waiting_for_peer", phase="negotiation", account_id=%binding.account, actor_id=%binding.actor));
    let negotiated_at = std::time::Instant::now();
    let mut dirty = false;
    let result = async {
        let local = synchronization::state(store).await?;
        send(
            socket,
            stop,
            ClientMessage::Hello {
                version: messages::VERSION,
                checkpoint: local.checkpoint.clone(),
                baselines: local.baselines,
                lineage: local.lineage.clone(),
            },
        )
        .await?;
        let message = response(socket, store, &mut dirty, connection).await?;
        match &message {
            ServerMessage::Hello {
                version, identity, ..
            } if *version == messages::VERSION
                && identity.account == binding.account
                && identity.actor == binding.actor =>
            {
                Ok(message)
            }
            _ => anyhow::bail!(
                "Server synchronization identity or protocol does not match enrollment"
            ),
        }
    }
    .instrument(negotiation.clone())
    .await;
    let outcome = if stop.is_cancelled() {
        "cancelled"
    } else if result.is_ok() {
        "completed"
    } else {
        "failed"
    };
    metrics::histogram!(crate::synchronization::PHASE_DURATION_METRIC, "phase" => "negotiation", "outcome" => outcome).record(negotiated_at.elapsed().as_secs_f64());
    if !stop.is_cancelled()
        && let Err(error) = &result
    {
        negotiation.set_status(opentelemetry::trace::Status::error(""));
        negotiation.set_attribute("error.type", crate::telemetry::failure::error_class(error));
        negotiation.in_scope(|| tracing::warn!(event="sync.negotiation.failed", account_id=%binding.account, actor_id=%binding.actor, error.type=crate::telemetry::failure::error_class(error)));
    }
    let ServerMessage::Hello {
        checkpoint,
        baselines,
        lineage,
        ..
    } = result?
    else {
        anyhow::bail!("Negotiation did not return a server checkpoint");
    };
    drop(negotiation);
    drop(negotiation_guard);
    let mut server_coverage = clacks_sync::transfer::discovery::target_coverage(
        &checkpoint,
        &clacks_sync::account::EvidenceView {
            lineage: &lineage,
            baselines: &baselines,
        },
    )?;
    let mut exchange = 1_u64;
    loop {
        let exchange_guard = connection.gate.exchange(stop).await?;
        send(socket, stop, ClientMessage::DirectoryConfiguration).await?;
        // Mark local notifications consumed before discovery; writes during it remain visible afterward.
        changes.borrow_and_update();
        dirty = false;
        let local = synchronization::state(store).await?;
        if let Some(ancestry) = &local.lineage {
            let returned = tracing::info_span!(parent: None, "desktop.sync.return", account_id=%binding.account, actor_id=%binding.actor);
            returned.add_link(connection.trace.clone());
            let evidence = clacks_sync::account::EvidenceView {
                lineage: ancestry,
                baselines: &local.baselines,
            };
            if let Some(known) = &local.checkpoint {
                let complete = clacks_sync::transfer::discovery::target_coverage(known, &evidence)?;
                if complete.entries().any(|(timeline, revision)| {
                    !server_coverage.covers(clacks_sync::record::Position { timeline, revision })
                }) && let Some(recovered) = return_accepted(
                    socket,
                    store,
                    clacks_client_store::discovery::Source::start(store).await?,
                    &mut dirty,
                    connection,
                )
                .instrument(returned.clone())
                .await?
                {
                    server_coverage = server_coverage.join(
                        &clacks_sync::transfer::discovery::target_coverage(&recovered, &evidence)?,
                    );
                }
            }
            return_accepted(
                socket,
                store,
                clacks_client_store::discovery::Source::repair(store, &server_coverage).await?,
                &mut dirty,
                connection,
            )
            .instrument(returned)
            .await?;
        }
        let span = tracing::info_span!(parent: None, "desktop.sync.inbound", account_id=%binding.account, actor_id=%binding.actor, exchange);
        span.add_link(connection.trace.clone());
        span.in_scope(|| tracing::info!(event="sync.waiting_for_peer", phase="inbound", account_id=%binding.account, actor_id=%binding.actor, exchange));
        let started = std::time::Instant::now();
        let result = inbound(socket, store, exchange, &mut dirty, connection)
            .instrument(span.clone())
            .await;
        let outcome = if stop.is_cancelled() {
            "cancelled"
        } else if result.is_ok() {
            "completed"
        } else {
            "failed"
        };
        metrics::histogram!(crate::synchronization::PHASE_DURATION_METRIC, "phase" => "inbound", "outcome" => outcome).record(started.elapsed().as_secs_f64());
        if !stop.is_cancelled()
            && let Err(error) = &result
        {
            span.set_status(opentelemetry::trace::Status::error(""));
            span.set_attribute("error.type", crate::telemetry::failure::error_class(error));
        }
        let issued = result?;
        drop(span);

        let received = synchronization::state(store).await?;
        let ancestry = received
            .lineage
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Received source has no lineage"))?;
        server_coverage = clacks_sync::transfer::discovery::target_coverage(
            &issued,
            &clacks_sync::account::EvidenceView {
                lineage: ancestry,
                baselines: &received.baselines,
            },
        )?;
        updates.send_replace(Some(Arc::new(
            clacks_client_store::settings::snapshot(store).await?,
        )));
        // No local proposal or frozen retry is sent until inbound completion above commits.
        let span = tracing::info_span!(parent: None, "desktop.sync.publish", account_id=%binding.account, actor_id=%binding.actor, exchange);
        span.add_link(connection.trace.clone());
        let started = std::time::Instant::now();
        let result = pending::publish(socket, store, &mut dirty, connection)
            .instrument(span.clone())
            .await;
        let outcome = if stop.is_cancelled() {
            "cancelled"
        } else if result.is_ok() {
            "completed"
        } else {
            "failed"
        };
        metrics::histogram!(crate::synchronization::PHASE_DURATION_METRIC, "phase" => "publication", "outcome" => outcome).record(started.elapsed().as_secs_f64());
        if !stop.is_cancelled()
            && let Err(error) = &result
        {
            span.set_status(opentelemetry::trace::Status::error(""));
            span.set_attribute("error.type", crate::telemetry::failure::error_class(error));
        }
        result?;
        drop(span);

        clacks_client_store::retention::reclaim_completed_cycle(store).await?;
        updates.send_replace(Some(Arc::new(
            clacks_client_store::settings::snapshot(store).await?,
        )));
        exchange = exchange
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Exchange sequence exhausted"))?;
        drop(exchange_guard);
        if dirty || changes.has_changed()? {
            continue;
        }
        tracing::info!(event="sync.idle", account_id=%binding.account, actor_id=%binding.actor, exchange);
        tokio::select! {
            _ = stop.cancelled() => return Ok(()),
            result = changes.changed() => {result?;},
            result = receive(socket, connection) => match result? {ServerMessage::Changed => {}, ServerMessage::Failure {kind,message} => return Err(ServerFailure {kind,message}.into()), _ => anyhow::bail!("Unexpected idle synchronization message")},
        }
    }
}

/// Discover and durably complete a fixed inbound target before acknowledging it.
pub(super) async fn inbound(
    socket: &mut Socket,
    store: &ClientStore,
    exchange: u64,
    dirty: &mut bool,
    connection: &ConnectionContext,
) -> anyhow::Result<clacks_sync::account::InstalledCheckpoint> {
    let stop = &connection.stop;
    let local = synchronization::state(store).await?;
    send(
        socket,
        stop,
        ClientMessage::Transfer {
            exchange,
            message: TransferMessage::Request {
                after: local.checkpoint,
            },
        },
    )
    .await?;
    let ServerMessage::Transfer {
        exchange: id,
        message: TransferMessage::Start(start),
    } = response(socket, store, dirty, connection).await?
    else {
        anyhow::bail!("Server did not start requested discovery");
    };
    anyhow::ensure!(id == exchange, "Wrong discovery exchange");
    let mut receipt = transfer::begin(store, &start).await?;
    loop {
        let ServerMessage::Transfer {
            exchange: id,
            message,
        } = response(socket, store, dirty, connection).await?
        else {
            anyhow::bail!("Unexpected discovery response");
        };
        anyhow::ensure!(id == exchange, "Wrong discovery exchange");
        match message {
            TransferMessage::Page(page) => {
                transfer::install_page(store, &mut receipt, &page).await?;
                send(
                    socket,
                    stop,
                    ClientMessage::Transfer {
                        exchange,
                        message: TransferMessage::PageAck {
                            sequence: page.sequence,
                        },
                    },
                )
                .await?;
            }
            TransferMessage::End(end) => {
                let completed = transfer::complete(store, &receipt, &end).await?;
                anyhow::ensure!(
                    end.source
                        .coverage
                        .entries()
                        .all(|(timeline, revision)| completed
                            .coverage
                            .covers(clacks_sync::record::Position { timeline, revision })),
                    "Local completion omitted issued source coverage"
                );
                send(
                    socket,
                    stop,
                    ClientMessage::Transfer {
                        exchange,
                        message: TransferMessage::CoverageAck(end.source.clone()),
                    },
                )
                .await?;
                return Ok(end.source);
            }
            _ => anyhow::bail!("Unexpected discovery operation"),
        }
    }
}

/// Return original accepted envelopes through the same discovered-state protocol, never as new pending edits.
async fn return_accepted(
    socket: &mut Socket,
    store: &ClientStore,
    mut source: clacks_client_store::discovery::Source,
    dirty: &mut bool,
    connection: &ConnectionContext,
) -> anyhow::Result<Option<clacks_sync::account::InstalledCheckpoint>> {
    let stop = &connection.stop;
    let certified = source.start.is_some();
    let exchange = 0;
    if let Some(start) = source.start.clone() {
        send(
            socket,
            stop,
            ClientMessage::Transfer {
                exchange,
                message: TransferMessage::Start(start),
            },
        )
        .await?;
    }
    while let Some(page) = source.page(store, 64).await? {
        let sequence = page.sequence;
        send(
            socket,
            stop,
            ClientMessage::Transfer {
                exchange,
                message: if certified {
                    TransferMessage::Page(page)
                } else {
                    TransferMessage::Facts(page)
                },
            },
        )
        .await?;
        tracing::info!(event="sync.waiting_for_peer", phase="returned_page_ack", account_id=%store.account_id(), exchange, sequence);
        anyhow::ensure!(
            matches!(response(socket, store, dirty, connection).await?, ServerMessage::Transfer {exchange:0, message:TransferMessage::PageAck {sequence:actual}} if actual == sequence),
            "Returned page was not acknowledged"
        );
    }
    if let Some(end) = source.end()? {
        send(
            socket,
            stop,
            ClientMessage::Transfer {
                exchange,
                message: TransferMessage::End(end.clone()),
            },
        )
        .await?;
        tracing::info!(event="sync.waiting_for_peer", phase="returned_coverage_ack", account_id=%store.account_id(), exchange);
        match response(socket, store, dirty, connection).await? {
            ServerMessage::Transfer {
                exchange: 0,
                message: TransferMessage::CoverageAck(checkpoint),
            } if checkpoint == end.source => return Ok(Some(checkpoint)),
            ServerMessage::Transfer {
                exchange: 0,
                message: TransferMessage::Deferred,
            } => {}
            _ => anyhow::bail!("Returned source was not acknowledged"),
        }
    } else {
        send(
            socket,
            stop,
            ClientMessage::Transfer {
                exchange,
                message: TransferMessage::FactsEnd,
            },
        )
        .await?;
        tracing::info!(event="sync.waiting_for_peer", phase="returned_facts_ack", account_id=%store.account_id(), exchange);
        anyhow::ensure!(
            matches!(
                response(socket, store, dirty, connection).await?,
                ServerMessage::Transfer {
                    exchange: 0,
                    message: TransferMessage::FactsEnd
                }
            ),
            "Accepted-facts response was not acknowledged"
        );
    }
    Ok(None)
}
