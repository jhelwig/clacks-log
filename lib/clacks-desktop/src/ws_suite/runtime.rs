//! Receiver I/O, pairing, asynchronous local work and graceful workspace drain.
use super::{
    Outputs, ReceiverAction, ReceiverRequest, ReceiverResponse, ReceiverView, ReceptionState,
    contact::CaptureContext,
    interfaces,
    pairing::{Admission, CandidateKey, Pairing, ReadyCapture},
    protocol, socket,
};
use clacks_client_store::{
    save::SaveOutcome,
    settings::SettingsSnapshot,
    store::ClientStore,
    ws_suite::{self, ReceiverSettings},
};
use clacks_model::ids::QsoId;
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

/// A committed reconfiguration awaiting the same admitted-work drain as shutdown.
struct PendingApply {
    /// Persisted next receiver intent.
    settings: ReceiverSettings,
    /// Already validated current interfaces for activation.
    interfaces: Vec<interfaces::InterfaceChoice>,
    /// Actual Apply outcome survives modal dismissal.
    reply: oneshot::Sender<ReceiverResponse>,
}

/// Outcomes of bounded native reads and saves, polled alongside socket readiness.
enum Completion {
    /// Candidate's one captured local context, or its read failure.
    Context {
        /// Candidate retained while the snapshot is acquired.
        key: CandidateKey,
        /// Original receipt instant for rejected-read timing.
        received: Instant,
        /// Captured context or a bounded safe diagnostic.
        result: Result<CaptureContext, String>,
    },
    /// Save and following read-refresh have distinct outcomes.
    Saved {
        /// Saving candidate still suppresses counterpart/repeated packets.
        key: CandidateKey,
        /// Stable native identity used by the save.
        id: QsoId,
        /// First receipt time for workflow latency.
        received: Instant,
        /// Bounded selected representation label.
        format: &'static str,
        /// Committed/acknowledged native save outcome.
        result: Result<Vec<String>, String>,
        /// Post-commit refresh independently affects history presentation.
        snapshot: Result<SettingsSnapshot, String>,
    },
}

/// Enumerate on an ordinary blocking worker without blocking native presentation.
async fn interfaces() -> Result<Vec<interfaces::InterfaceChoice>, String> {
    tokio::task::spawn_blocking(interfaces::enumerate)
        .await
        .map_err(|_| "Network interface enumeration stopped unexpectedly.".to_string())?
        .map_err(|e| e.to_string())
}

/// Activate committed intent; failure retains the preference and no obsolete socket.
fn activate(
    settings: &ReceiverSettings,
    choices: &[interfaces::InterfaceChoice],
    view: &mut ReceiverView,
) -> Option<tokio::net::UdpSocket> {
    view.settings = Some(settings.clone());
    if !settings.enabled {
        view.reception = ReceptionState::Off;
        tracing::info!(event = "ws_suite.receiver_stopped");
        return None;
    }
    match socket::bind(&settings.connection, choices) {
        Ok(socket) => {
            view.reception = ReceptionState::Listening;
            tracing::info!(event = "ws_suite.receiver_started");
            Some(socket)
        }
        Err(error) => {
            view.reception = ReceptionState::Failed(format!(
                "Could not start reception: {error}. Correct the settings or Apply again to retry."
            ));
            tracing::error!(event = "ws_suite.receiver_failed", stage = "activation");
            None
        }
    }
}

/// Admit a packet and launch only the first notification's local context read.
fn packet(
    bytes: &[u8],
    sender: SocketAddr,
    received: Instant,
    draining: bool,
    pairs: &mut Pairing,
    work: &mut FuturesUnordered<BoxFuture<'static, Completion>>,
    store: &ClientStore,
    outputs: &Outputs,
    lookup: &crate::directory::lookup::LookupClient,
) {
    let notice = match protocol::decode(bytes, sender) {
        Ok(Some(protocol::DecodedMessage::Completion(notice))) => notice,
        Ok(Some(protocol::DecodedMessage::Status(call))) => {
            if !draining {
                let _ = outputs.calls.send((lookup.clone(), call));
                if let Some(context) = outputs.context.get() {
                    context.request_repaint();
                }
            }
            return;
        }
        Ok(None) => return,
        Err(_) => {
            tracing::warn!(event="ws_suite.message_rejected",source_ip=%sender.ip(),reason="malformed_or_unsupported");
            return;
        }
    };
    if draining && !pairs.contains(&notice) {
        return;
    }
    match pairs.admit(notice, received) {
        Admission::New(key) => {
            tracing::debug!(name:"ws_suite.capture_admitted",source_ip=%key.source.sender,source_id=%key.source.instance);
            let store = store.clone();
            work.push(
                async move {
                    let result = clacks_client_store::settings::snapshot(&store)
                        .await
                        .and_then(|snapshot| CaptureContext::from_snapshot(&snapshot))
                        .map_err(|_| {
                            "Could not read current settings for the completed WS Suite contact."
                                .into()
                        });
                    Completion::Context {
                        key,
                        received,
                        result,
                    }
                }
                .boxed(),
            );
        }
        Admission::Joined(key) => {
            tracing::debug!(event="ws_suite.representation_joined",source_ip=%key.source.sender,source_id=%key.source.instance)
        }
        Admission::Suppressed => {}
    }
}

/// Record one terminal candidate outcome using bounded metric dimensions.
fn measure(received: Instant, format: &'static str, outcome: &'static str) {
    metrics::counter!("clacks_ws_suite_captures_total","format"=>format,"outcome"=>outcome)
        .increment(1);
    metrics::histogram!("clacks_ws_suite_capture_duration_seconds","format"=>format,"outcome"=>outcome).record(received.elapsed().as_secs_f64());
}

/// Start a transactional contact save without blocking packet reception.
fn save(
    capture: ReadyCapture,
    store: &ClientStore,
    lookup: &crate::directory::lookup::LookupClient,
    changes: &watch::Sender<()>,
    work: &mut FuturesUnordered<BoxFuture<'static, Completion>>,
    pairs: &mut Pairing,
    view: &mut ReceiverView,
    outputs: &Outputs,
) {
    let format = capture
        .format
        .map_or("none", protocol::CompletionFormat::label);
    let projection = match capture.projection {
        Ok(projection) => projection,
        Err(error) => {
            tracing::warn!(event="ws_suite.capture_rejected",qso_id=%capture.id,source_ip=%capture.key.source.sender,source_id=%capture.key.source.instance);
            view.capture_notice = Some(error);
            outputs.publish(view.clone());
            measure(capture.received, format, "rejected");
            pairs.settle(&capture.key, false);
            return;
        }
    };
    let store = store.clone();
    let key = clacks_model::directory::values::LookupKey::from_recorded(
        &projection.qso.contacted_station().callsign,
    );
    let directory = crate::directory::lookup::persistence::prepare(
        lookup.clone(),
        store.clone(),
        capture.id,
        crate::directory::lookup::LookupDemand {
            handle: Some(lookup.request(key.clone())),
            key,
            due: Instant::now(),
        },
        changes.clone(),
    );
    let span = tracing::debug_span!(
        "ws_suite.capture_save",
        qso_id = %capture.id,
        source_ip = %capture.key.source.sender,
        source_id = %capture.key.source.instance,
    );
    work.push(
        async move {
            let result = match clacks_client_store::save::contacts::save(
                &store, None, projection.qso, directory.ready_snapshot().map(|snapshot| *snapshot),
            ).await {
                Ok(SaveOutcome::Saved(_) | SaveOutcome::Unchanged(_)) => Ok(projection.warnings),
                Ok(_) => Err("The completed WS Suite contact could not be saved because its local record changed.".into()),
                Err(error) => {
                    tracing::error!(
                        event = "ws_suite.capture_save_failed",
                        qso_id = %capture.id,
                        error.type = crate::telemetry::failure::error_class(&error),
                    );
                    Err("Could not save the completed WS Suite contact. It has not been captured.".into())
                }
            };
            directory.settle(result.is_ok());

            let snapshot = if result.is_ok() {
                clacks_client_store::settings::snapshot(&store).await.map_err(|_| {
                    "The contact was saved, but history could not be refreshed.".into()
                })
            } else {
                Err(String::new())
            };
            Completion::Saved {
                key: capture.key,
                id: capture.id,
                received: capture.received,
                format,
                result,
                snapshot,
            }
        }.instrument(span).boxed(),
    );
}

/// Own receiver I/O, admitted context/save futures and latest identities for one workspace.
/// Apply and shutdown drain the same pending candidates before releasing the socket/store.
/// Return drained recognition state so a failed enrollment can resume the same workspace.
pub(crate) async fn run(
    store: ClientStore,
    lookup: crate::directory::lookup::LookupClient,
    mut commands: mpsc::Receiver<ReceiverRequest>,
    outputs: Outputs,
    settings_updates: watch::Sender<Option<Arc<SettingsSnapshot>>>,
    changes: watch::Sender<()>,
    mut pairs: Pairing,
    stop: CancellationToken,
) -> anyhow::Result<Pairing> {
    metrics::describe_counter!(
        "clacks_ws_suite_captures_total",
        "Terminal completed-contact capture outcomes by selected format and outcome"
    );
    metrics::describe_histogram!(
        "clacks_ws_suite_capture_duration_seconds",
        metrics::Unit::Seconds,
        "First receipt to terminal local capture outcome"
    );
    let mut view = ReceiverView::default();
    let mut listener = None;
    match ws_suite::load(&store).await {
        Ok(settings) => {
            if settings.enabled {
                match interfaces().await {
                    Ok(choices) => listener = activate(&settings, &choices, &mut view),
                    Err(error) => {
                        view.settings = Some(settings);
                        view.reception = ReceptionState::Failed(error);
                        tracing::error!(event = "ws_suite.receiver_failed", stage = "interfaces");
                    }
                }
            } else {
                view.settings = Some(settings);
            }
        }
        Err(_) => {
            tracing::error!(event = "ws_suite.settings_load_failed");
            view.reception =
                ReceptionState::Failed("Could not read WS Suite receiver settings.".into());
        }
    }
    outputs.publish(view.clone());
    let mut work = FuturesUnordered::new();
    let mut pending: Option<PendingApply> = None;
    let mut closing = false;
    let mut buffer = vec![0; 65_535];
    loop {
        if stop.is_cancelled() {
            closing = true;
            commands.close();
        }
        // Drain already-readable datagrams before deciding that a pairing deadline expired.
        if let Some(socket) = &listener {
            loop {
                match socket.try_recv_from(&mut buffer) {
                    Ok((length, sender)) => {
                        if let Some(bytes) = buffer.get(..length) {
                            packet(
                                bytes,
                                sender,
                                Instant::now(),
                                closing || pending.is_some(),
                                &mut pairs,
                                &mut work,
                                &store,
                                &outputs,
                                &lookup,
                            );
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        listener = None;
                        view.reception = ReceptionState::Failed(
                            "WS Suite reception stopped. Apply the receiver settings to retry."
                                .into(),
                        );
                        outputs.publish(view.clone());
                        tracing::error!(event = "ws_suite.receiver_failed", stage = "receive");
                        break;
                    }
                }
            }
        }
        for capture in pairs.take_ready(Instant::now()) {
            save(
                capture, &store, &lookup, &changes, &mut work, &mut pairs, &mut view, &outputs,
            );
        }
        if pairs.is_empty() && work.is_empty() {
            if closing {
                break;
            }
            if let Some(apply) = pending.take() {
                drop(listener.take());
                listener = activate(&apply.settings, &apply.interfaces, &mut view);
                outputs.publish(view.clone());
                let _ = apply.reply.send(ReceiverResponse::Applied(view.clone()));
                if let Some(context) = outputs.context.get() {
                    context.request_repaint();
                }
            }
        }
        let deadline = pairs.next_deadline();
        tokio::select! {
            biased;
            _ = stop.cancelled(), if !closing => {
                closing = true;
                commands.close();
            }
            request = commands.recv(), if !closing && pending.is_none() => {
                let Some(request) = request else {
                    closing = true;
                    continue;
                };
                match request.action {
                    ReceiverAction::Interfaces => {
                        let result = interfaces().await;
                        let _ = request.reply.send(ReceiverResponse::Interfaces(result));
                    }
                    ReceiverAction::Apply { next } => {
                        let choices = if next.enabled { interfaces().await } else { Ok(vec![]) };
                        let validation = choices.as_ref().map_err(Clone::clone).and_then(|choices| {
                            if next.enabled {
                                socket::validate(&next.connection, choices).map_err(|error| error.to_string())
                            } else {
                                Ok(())
                            }
                        });
                        if let Err(error) = validation {
                            let _ = request.reply.send(ReceiverResponse::Failed(error));
                        } else {
                            match ws_suite::save(&store, &next).await {
                                Err(_) => {
                                    tracing::error!(event = "ws_suite.settings_save_failed");
                                    let _ = request.reply.send(ReceiverResponse::Failed(
                                        "Could not save WS Suite receiver settings. Your changes have not been applied.".into(),
                                    ));
                                }
                                Ok(saved) => {
                                    if saved.changed || matches!(view.reception, ReceptionState::Failed(_)) {
                                        pending = Some(PendingApply {
                                            settings: saved.settings,
                                            interfaces: choices.unwrap_or_default(),
                                            reply: request.reply,
                                        });
                                    } else {
                                        let _ = request.reply.send(ReceiverResponse::Applied(view.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(context) = outputs.context.get() {
                    context.request_repaint();
                }
            }
            ready = async {
                match &listener {
                    Some(socket) => socket.readable().await,
                    None => std::future::pending().await,
                }
            } => {
                if ready.is_err() {
                    listener = None;
                    view.reception = ReceptionState::Failed(
                        "WS Suite reception stopped. Apply the receiver settings to retry.".into(),
                    );
                    outputs.publish(view.clone());
                    tracing::error!(event = "ws_suite.receiver_failed", stage = "readiness");
                }
            }
            completed = work.next(), if !work.is_empty() => {
                match completed {
                    Some(Completion::Context { key, received, result }) => match result {
                        Ok(context) => pairs.attach_context(&key, context)?,
                        Err(error) => {
                            pairs.settle(&key, false);
                            view.capture_notice = Some(error);
                            outputs.publish(view.clone());
                            measure(received, "none", "rejected");
                            tracing::error!(
                                event = "ws_suite.capture_context_failed",
                                source_ip = %key.source.sender,
                                source_id = %key.source.instance,
                            );
                        }
                    },
                    Some(Completion::Saved { key, id, received, format, result, snapshot }) => {
                        let saved = result.is_ok();
                        pairs.settle(&key, saved);
                        match result {
                            Ok(warnings) => {
                                changes.send_replace(());
                                tracing::info!(
                                    name: "ws_suite.capture_committed",
                                    event = "ws_suite.capture_committed",
                                    qso_id = %id,
                                    source_ip = %key.source.sender,
                                    source_id = %key.source.instance,
                                    format,
                                );
                                if !warnings.is_empty() {
                                    tracing::warn!(event = "ws_suite.capture_conversion_warning", qso_id = %id, fields = ?warnings);
                                    view.capture_notice.get_or_insert_with(|| format!(
                                        "Contact captured; these supplied fields could not be recorded: {}.", warnings.join(", "),
                                    ));
                                }
                                match snapshot {
                                    Ok(snapshot) => { settings_updates.send_replace(Some(Arc::new(snapshot))); }
                                    Err(error) => {
                                        view.capture_notice = Some(error);
                                        tracing::error!(event = "ws_suite.capture_refresh_failed", qso_id = %id);
                                    }
                                }
                            }
                            Err(error) => {
                                view.capture_notice = Some(error);
                                tracing::error!(
                                    event = "ws_suite.capture_failed",
                                    qso_id = %id,
                                    source_ip = %key.source.sender,
                                    source_id = %key.source.instance,
                                );
                            }
                        }
                        measure(received, format, if saved { "saved" } else { "failed" });
                        outputs.publish(view.clone());
                    }
                    None => {}
                }
            }
            _ = async {
                match deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {}
        }
    }
    if let Some(apply) = pending {
        let _ = apply.reply.send(ReceiverResponse::Closing);
    }
    while let Ok(request) = commands.try_recv() {
        let _ = request.reply.send(ReceiverResponse::Closing);
    }
    drop(listener);
    tracing::info!(event = "ws_suite.receiver_stopped");
    Ok(pairs)
}
