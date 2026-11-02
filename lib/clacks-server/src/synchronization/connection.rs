//! Ordered exchange dispatch; network waits never own a database transaction.

mod failure;

#[cfg(test)]
mod tests;

use crate::{
    enrollment::AuthorizedClient,
    runtime::ServerState,
    store::{discovery::Source, metadata::account, synchronization},
};
use axum::extract::ws::{Message, WebSocket};
use clacks_sync::{
    protocol::{
        ClientIdentity,
        messages::{
            self, ClientMessage, FailureKind, ProposalOutcome, ServerMessage, TransferMessage,
        },
    },
    transfer::receipt::ReceiptState,
};
use failure::InvalidMessage;

/// Send only an already determined native outcome; socket loss cannot undo its transaction.
async fn send(
    socket: &mut WebSocket,
    stop: &tokio_util::sync::CancellationToken,
    expires: time::OffsetDateTime,
    value: ServerMessage,
) -> anyhow::Result<()> {
    let now = time::OffsetDateTime::now_utc();
    anyhow::ensure!(expires > now, "Client authorization expired");
    let remaining = std::time::Duration::try_from(expires - now)?;
    let frame = Message::Text(serde_json::to_string(&value)?.into());
    tokio::select! {
        biased;
        _ = stop.cancelled() => anyhow::bail!("Synchronization stopped"),
        _ = tokio::time::sleep(remaining) => anyhow::bail!("Client authorization expired"),
        result = socket.send(frame) => { result?; Ok(()) },
    }
}

/// One source awaiting ordered durable page acknowledgment or a final coverage acknowledgment.
struct Sending {
    /// Correlation owned by the requesting peer.
    exchange: u64,
    /// Fixed target and immutable enumeration cursor.
    source: Source,
    /// Last emitted page, if its durable acknowledgment is outstanding.
    awaiting_page: Option<u64>,
    /// Source exhaustion has been sent; only complete coverage may now be acknowledged.
    ended: bool,
}

/// Inbound certified discovery state, separate from record-only accepted repair.
struct Receiving {
    /// Peer-owned connection correlation.
    exchange: u64,
    /// Registered obligations and actual committed progress.
    receipt: ReceiptState,
}

/// Ordered record-only repair; its sequence never represents complete-prefix coverage.
struct FactExchange {
    /// Peer-owned connection correlation.
    exchange: u64,
    /// Next expected page.
    next: u64,
}

/// Advertise actual account authority and interpretation from one consistent read.
async fn hello(state: &ServerState, client: &AuthorizedClient) -> anyhow::Result<ServerMessage> {
    let mut transaction = state
        .store
        .pool
        .begin_with("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    crate::oauth::resource::check_current(&mut transaction, client).await?;
    let account = account::load(&mut transaction, client.account).await?;
    let interpretation = account::interpretation(&mut transaction, client.account).await?;
    let checkpoint = synchronization::checkpoint(&account);
    transaction.commit().await?;
    Ok(ServerMessage::Hello {
        version: messages::VERSION,
        identity: ClientIdentity {
            account: client.account,
            actor: client.actor,
        },
        checkpoint,
        baselines: interpretation.baselines,
        lineage: interpretation.lineage,
    })
}

/// Emit the next page or the checked terminal assertion after the previous durable acknowledgment.
async fn next(
    socket: &mut WebSocket,
    state: &ServerState,
    client: &AuthorizedClient,
    sending: &mut Sending,
) -> anyhow::Result<()> {
    let message = match sending.source.page(&state.store, client, 64).await? {
        Some(page) => {
            sending.awaiting_page = Some(page.sequence);
            TransferMessage::Page(page)
        }
        None => {
            sending.ended = true;
            TransferMessage::End(sending.source.end()?)
        }
    };
    send(
        socket,
        &state.sync_stop,
        client.expires_at,
        ServerMessage::Transfer {
            exchange: sending.exchange,
            message,
        },
    )
    .await
}

/// Own the socket through failure reporting while allowing admitted storage work to finish.
pub(super) async fn run(mut socket: WebSocket, state: ServerState, client: AuthorizedClient) {
    let Err(error) = exchange(&mut socket, &state, &client).await else {
        return;
    };
    if state.sync_stop.is_cancelled() || client.expires_at <= time::OffsetDateTime::now_utc() {
        return;
    }
    if error.downcast_ref::<axum::Error>().is_some() {
        tracing::warn!(event="sync.connection.interrupted", account_id=%client.account, actor_id=%client.actor, "Synchronization transport interrupted");
        return;
    }
    let (kind, message) = failure::classify(&error);
    if kind == FailureKind::Unavailable {
        let database_code = error
            .downcast_ref::<sqlx::Error>()
            .and_then(|error| error.as_database_error())
            .and_then(|error| error.code());
        tracing::error!(event="sync.connection.failed", account_id=%client.account, actor_id=%client.actor, ?kind, database_code=?database_code, error.type=crate::store::diagnostics::error_class(&error), reason=%message);
    } else {
        tracing::warn!(event="sync.connection.rejected", account_id=%client.account, actor_id=%client.actor, ?kind, reason=%message);
    }
    // A failed diagnostic write cannot undo committed work or initiate another write.
    let _ = send(
        &mut socket,
        &state.sync_stop,
        client.expires_at,
        ServerMessage::Failure { kind, message },
    )
    .await;
}

/// Exchange native messages while each store operation owns its actual transaction.
async fn exchange(
    socket: &mut WebSocket,
    state: &ServerState,
    client: &AuthorizedClient,
) -> anyhow::Result<()> {
    // Subscribe before the first state read so a change cannot fall between discovery and wakeup.
    let mut changes = state.store.changes.subscribe();
    let mut negotiated = false;
    let mut ready_to_publish = false;
    let mut sending: Option<Sending> = None;
    let mut receiving: Option<Receiving> = None;
    let mut facts: Option<FactExchange> = None;
    let remaining = (client.expires_at - time::OffsetDateTime::now_utc())
        .try_into()
        .unwrap_or(std::time::Duration::ZERO);
    let expiry = tokio::time::sleep(remaining);
    tokio::pin!(expiry);
    let mut reported_wait = None;
    loop {
        if negotiated {
            let phase = if sending.is_some() {
                "awaiting_transfer_ack"
            } else if receiving.is_some() {
                "awaiting_transfer_page"
            } else if facts.is_some() {
                "awaiting_repair_facts"
            } else {
                "idle"
            };
            use opentelemetry::trace::TraceContextExt;
            if reported_wait != Some(phase) {
                reported_wait = Some(phase);
                tracing::info!(event="sync.peer_wait", account_id=%client.account, actor_id=%client.actor, phase, connection_trace_id=%opentelemetry::Context::current().span().span_context().trace_id());
            }
        }
        let frame = tokio::select! {

            _ = state.sync_stop.cancelled() => break,
            _ = &mut expiry => break,
            change = changes.recv(), if negotiated => {
                match change {
                    Ok(account) if account != client.account => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    _ => {
                        let mut connection = state.store.pool.acquire().await?;
                        crate::oauth::resource::check_current(&mut connection, client).await?;
                        drop(connection);
                        send(socket, &state.sync_stop, client.expires_at, ServerMessage::Changed).await?;
                        continue;
                    }
                }
            },
            frame = socket.recv() => frame,
        };
        let Some(frame) = frame else { break };
        let frame = frame?;
        let text = match frame {
            Message::Text(text) => text,
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Close(_) => break,
            _ => anyhow::bail!(InvalidMessage {
                reason: "expected native JSON text frame"
            }),
        };
        reported_wait = None;
        let message: ClientMessage = serde_json::from_str(&text).map_err(|_| InvalidMessage {
            reason: "invalid native synchronization message",
        })?;
        use opentelemetry::trace::TraceContextExt;
        use tracing::Instrument;
        use tracing_opentelemetry::OpenTelemetrySpanExt;
        let exchange_id = message.exchange_id();
        let phase = message.diagnostic_phase();
        let span = tracing::info_span!(parent: None, "server.sync.dispatch", phase, exchange=exchange_id, account_id=%client.account, actor_id=%client.actor);
        span.add_link(
            opentelemetry::Context::current()
                .span()
                .span_context()
                .clone(),
        );
        let started = std::time::Instant::now();
        span.in_scope(|| tracing::info!(event="sync.work.admitted", phase, account_id=%client.account, actor_id=%client.actor));
        let result = async {
            match message {
                ClientMessage::Hello {
                    version,
                    checkpoint,
                    baselines,
                    lineage,
                } => {
                    anyhow::ensure!(
                        !negotiated,
                        InvalidMessage {
                            reason: "duplicate negotiation"
                        }
                    );
                    if version != messages::VERSION {
                        send(
                            socket,
                            &state.sync_stop,
                            client.expires_at,
                            ServerMessage::Failure {
                                kind: FailureKind::Incompatible,
                                message: "Synchronization protocol upgrade required".into(),
                            },
                        )
                        .await?;
                        return Ok(true);
                    }
                    synchronization::recovery::negotiate(
                        &state.store,
                        client,
                        checkpoint.as_ref(),
                        &baselines,
                        lineage.as_ref(),
                    )
                    .await?;
                    send(
                        socket,
                        &state.sync_stop,
                        client.expires_at,
                        hello(state, client).await?,
                    )
                    .await?;
                    negotiated = true;
                }
                ClientMessage::Propose { request, change } => {
                    anyhow::ensure!(
                        negotiated && ready_to_publish && sending.is_none() && receiving.is_none(),
                        InvalidMessage {
                            reason: "proposal before completed inbound exchange"
                        }
                    );
                    let outcome =
                        match synchronization::accept_pending(&state.store, client, &change).await?
                        {
                            clacks_sync::acceptance::AcceptanceDecision::Commit(commit) => {
                                ProposalOutcome::Accepted(commit.accepted)
                            }
                            clacks_sync::acceptance::AcceptanceDecision::Covered(value) => {
                                ProposalOutcome::Covered(value)
                            }
                            clacks_sync::acceptance::AcceptanceDecision::Stale(value) => {
                                ProposalOutcome::Stale(value)
                            }
                            clacks_sync::acceptance::AcceptanceDecision::Rejected(reason) => {
                                ProposalOutcome::Rejected(reason)
                            }
                        };
                    send(
                        socket,
                        &state.sync_stop,
                        client.expires_at,
                        ServerMessage::Proposal { request, outcome },
                    )
                    .await?;
                }
                ClientMessage::Transfer { exchange, message } => {
                    anyhow::ensure!(
                        negotiated,
                        InvalidMessage {
                            reason: "transfer before negotiation"
                        }
                    );
                    match message {
                        TransferMessage::Request { after } => {
                            ready_to_publish = false;
                            anyhow::ensure!(
                                sending.is_none() && receiving.is_none(),
                                InvalidMessage {
                                    reason: "overlapping discovery"
                                }
                            );
                            anyhow::ensure!(
                                after
                                    .as_ref()
                                    .is_none_or(|checkpoint| checkpoint.account == client.account),
                                InvalidMessage {
                                    reason: "Discovery checkpoint belongs to another account"
                                }
                            );
                            let source =
                                Source::start(&state.store, client, after.as_ref()).await?;

                            send(
                                socket,
                                &state.sync_stop,
                                client.expires_at,
                                ServerMessage::Transfer {
                                    exchange,
                                    message: TransferMessage::Start(source.start.clone()),
                                },
                            )
                            .await?;
                            let mut source = Sending {
                                exchange,
                                source,
                                awaiting_page: None,
                                ended: false,
                            };
                            next(socket, state, client, &mut source).await?;
                            sending = Some(source);
                        }
                        TransferMessage::PageAck { sequence } => {
                            let source = sending.as_mut().ok_or(InvalidMessage {
                                reason: "page acknowledgment without discovery",
                            })?;
                            anyhow::ensure!(
                                source.exchange == exchange
                                    && source.awaiting_page == Some(sequence)
                                    && !source.ended,
                                InvalidMessage {
                                    reason: "invalid page acknowledgment"
                                }
                            );
                            source.awaiting_page = None;
                            next(socket, state, client, source).await?;
                        }
                        TransferMessage::CoverageAck(completed) => {
                            let source = sending.as_ref().ok_or(InvalidMessage {
                                reason: "coverage acknowledgment without discovery",
                            })?;
                            anyhow::ensure!(
                                source.exchange == exchange
                                    && source.ended
                                    && source.awaiting_page.is_none()
                                    && completed == source.source.start.source,
                                InvalidMessage {
                                    reason: "coverage was not issued/completed by this exchange"
                                }
                            );
                            synchronization::acknowledge(&state.store, client, &completed).await?;
                            sending = None;
                            ready_to_publish = true;
                        }
                        TransferMessage::Start(start) => {
                            anyhow::ensure!(
                                sending.is_none()
                                    && receiving.is_none()
                                    && start.source.account == client.account,
                                InvalidMessage {
                                    reason: "invalid returned accepted source"
                                }
                            );
                            receiving = Some(Receiving {
                                exchange,
                                receipt: ReceiptState::begin(&start).map_err(|_| {
                                    InvalidMessage {
                                        reason: "Invalid returned discovery start",
                                    }
                                })?,
                            });
                        }
                        TransferMessage::Page(page) => {
                            let Receiving {
                                exchange: id,
                                receipt,
                            } = receiving.as_mut().ok_or(InvalidMessage {
                                reason: "page without source",
                            })?;
                            anyhow::ensure!(
                                *id == exchange,
                                InvalidMessage {
                                    reason: "wrong incoming exchange"
                                }
                            );
                            let mut registered =
                                clacks_sync::transfer::receipt::register_page(receipt, &page)
                                    .map_err(|_| InvalidMessage {
                                        reason: "Invalid returned discovery page or retry",
                                    })?;
                            let committed =
                                synchronization::receipt::receive(&state.store, client, &page)
                                    .await?;

                            registered.staged = committed
                                .staged
                                .iter()
                                .map(clacks_sync::service::VersionReference::from)
                                .collect();
                            registered.completed_units.extend(committed.completed_units);
                            for input in committed.reviews {
                                send(
                                    socket,
                                    &state.sync_stop,
                                    client.expires_at,
                                    ServerMessage::Review {
                                        input: Box::new(input),
                                    },
                                )
                                .await?;
                            }
                            *receipt = registered;
                            send(
                                socket,
                                &state.sync_stop,
                                client.expires_at,
                                ServerMessage::Transfer {
                                    exchange,
                                    message: TransferMessage::PageAck {
                                        sequence: page.sequence,
                                    },
                                },
                            )
                            .await?;
                        }
                        TransferMessage::End(end) => {
                            let Receiving {
                                exchange: id,
                                receipt,
                            } = receiving.take().ok_or(InvalidMessage {
                                reason: "end without source",
                            })?;
                            anyhow::ensure!(
                                id == exchange,
                                InvalidMessage {
                                    reason: "wrong incoming exchange"
                                }
                            );
                            let complete = synchronization::receipt::complete_return(
                                &state.store,
                                client,
                                &receipt,
                                &end,
                            )
                            .await?;
                            let message = if complete {
                                TransferMessage::CoverageAck(end.source)
                            } else {
                                TransferMessage::Deferred
                            };
                            send(
                                socket,
                                &state.sync_stop,
                                client.expires_at,
                                ServerMessage::Transfer { exchange, message },
                            )
                            .await?;
                        }
                        TransferMessage::Facts(page) => {
                            anyhow::ensure!(
                                sending.is_none() && receiving.is_none(),
                                InvalidMessage {
                                    reason: "facts overlap discovery"
                                }
                            );
                            let expected = facts.get_or_insert(FactExchange { exchange, next: 0 });
                            anyhow::ensure!(
                                expected.exchange == exchange && expected.next == page.sequence,
                                InvalidMessage {
                                    reason: "invalid facts sequence"
                                }
                            );
                            let committed =
                                synchronization::receipt::receive(&state.store, client, &page)
                                    .await?;
                            for input in committed.reviews {
                                send(
                                    socket,
                                    &state.sync_stop,
                                    client.expires_at,
                                    ServerMessage::Review {
                                        input: Box::new(input),
                                    },
                                )
                                .await?;
                            }
                            expected.next = expected.next.checked_add(1).ok_or(InvalidMessage {
                                reason: "facts sequence exhausted",
                            })?;
                            send(
                                socket,
                                &state.sync_stop,
                                client.expires_at,
                                ServerMessage::Transfer {
                                    exchange,
                                    message: TransferMessage::PageAck {
                                        sequence: page.sequence,
                                    },
                                },
                            )
                            .await?;
                        }
                        TransferMessage::FactsEnd => {
                            if let Some(expected) = facts.take() {
                                anyhow::ensure!(
                                    expected.exchange == exchange,
                                    InvalidMessage {
                                        reason: "wrong facts exchange"
                                    }
                                );
                            }
                            send(
                                socket,
                                &state.sync_stop,
                                client.expires_at,
                                ServerMessage::Transfer {
                                    exchange,
                                    message: TransferMessage::FactsEnd,
                                },
                            )
                            .await?;
                        }
                        TransferMessage::Deferred => {
                            anyhow::bail!(InvalidMessage {
                                reason: "unexpected deferred response"
                            })
                        }
                    }
                }
            }
            Ok::<bool, anyhow::Error>(false)
        }
        .instrument(span.clone())
        .await;
        let outcome = if result.is_ok() {
            "completed"
        } else {
            "failed"
        };
        metrics::histogram!(crate::synchronization::SYNC_DISPATCH_DURATION_SECONDS_METRIC, "phase" => phase, "outcome" => outcome).record(started.elapsed().as_secs_f64());
        if let Err(error) = &result {
            span.set_attribute("error.type", crate::store::diagnostics::error_class(error));
            span.set_status(opentelemetry::trace::Status::error(""));
        }
        if result? {
            break;
        }
    }
    Ok(())
}
