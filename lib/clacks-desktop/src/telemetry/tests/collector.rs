//! Bounded Rust HTTP receiver for exercising the actual OTLP composition path.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

/// Captured synthetic wire request for protocol and header assertions.
pub(super) struct Request {
    /// HTTP signal path, including any synthetic test query.
    pub path: String,
    /// Bounded protobuf bytes sent by the production exporter.
    pub body: Vec<u8>,
    /// Synthetic authorization value used only by exact-byte credential tests.
    pub authorization: Option<String>,
}

/// HTTP behavior selected by a composition regression case.
#[derive(Clone, Copy)]
pub(super) enum Reply {
    /// Accept valid telemetry immediately.
    Accept,
    /// Reject with a synthetic secret-bearing body to exercise diagnostic exclusion.
    RejectSecret,
    /// Respond after the client's configured request deadline.
    Delayed,
    /// Successful protobuf response carrying a secret-bearing rejection message.
    PartialSecret,
}

/// Local receiver with deterministic termination and bounded capture storage.
pub(super) struct Collector {
    /// Listening loopback address selected by the OS.
    pub address: SocketAddr,
    /// Captured messages shared with assertions.
    pub requests: Arc<Mutex<Vec<Request>>>,
    /// Receiver exit request.
    stop: Arc<AtomicBool>,
    /// Joinable listener thread.
    task: Option<JoinHandle<anyhow::Result<()>>>,
}

impl Collector {
    /// Receive actual HTTP/protobuf exports; optionally echo a synthetic error body.
    pub fn start(reply: Reply) -> anyhow::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;

        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let captured = requests.clone();
        let stopping = stop.clone();
        let task = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let (stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };

                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;

                let mut reader = BufReader::new(stream);
                let mut first = String::new();
                reader.read_line(&mut first)?;
                let path = first
                    .split_whitespace()
                    .nth(1)
                    .ok_or_else(|| anyhow::anyhow!("missing HTTP path"))?
                    .to_owned();

                let mut length = None;
                let mut authorization = None;
                let mut header_bytes = first.len();

                loop {
                    let mut line = String::new();
                    let bytes = reader.read_line(&mut line)?;
                    anyhow::ensure!(bytes > 0, "incomplete HTTP headers");
                    header_bytes += bytes;
                    anyhow::ensure!(header_bytes <= 16384, "test request headers exceed limit");
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("authorization")
                    {
                        authorization = Some(value.trim().to_owned());
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = Some(value.trim().parse::<usize>()?);
                    }
                }

                let length = length.ok_or_else(|| anyhow::anyhow!("missing HTTP body length"))?;
                anyhow::ensure!(length <= 1_048_576, "test request body exceeds limit");
                let mut body = vec![0; length];
                reader.read_exact(&mut body)?;

                let mut captured = captured.lock().expect("receiver mutex");
                anyhow::ensure!(captured.len() < 100, "unexpected exporter request storm");
                let partial = partial_response(&path);
                captured.push(Request {
                    path,
                    body,
                    authorization,
                });
                drop(captured);

                if matches!(reply, Reply::Delayed) {
                    std::thread::sleep(Duration::from_millis(300));
                }

                let response = match reply {
                    Reply::RejectSecret => b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 15\r\nConnection: close\r\n\r\nSECRET_SENTINEL".to_vec(),
                    Reply::PartialSecret => {
                        let mut response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", partial.len()).into_bytes();
                        response.extend(partial);
                        response
                    }
                    _ => b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                };

                if let Err(error) = reader.get_mut().write_all(&response)
                    && !matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe
                            | std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::ConnectionAborted
                    )
                {
                    return Err(error.into());
                }
            }
            Ok(())
        });

        Ok(Self {
            address,
            requests,
            stop,
            task: Some(task),
        })
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(task) = self.task.take() {
            task.join()
                .expect("receiver thread")
                .expect("receiver result");
        }
    }
}

/// Independently encode actual signal-specific rejection messages with project Rust tooling.
fn partial_response(path: &str) -> Vec<u8> {
    use opentelemetry_proto::tonic::collector::{
        logs::v1::{ExportLogsPartialSuccess, ExportLogsServiceResponse},
        metrics::v1::{ExportMetricsPartialSuccess, ExportMetricsServiceResponse},
        trace::v1::{ExportTracePartialSuccess, ExportTraceServiceResponse},
    };
    use prost::Message;

    match path {
        "/v1/logs" => ExportLogsServiceResponse {
            partial_success: Some(ExportLogsPartialSuccess {
                rejected_log_records: 1,
                error_message: "SECRET_SENTINEL".into(),
            }),
        }
        .encode_to_vec(),
        "/v1/metrics" => ExportMetricsServiceResponse {
            partial_success: Some(ExportMetricsPartialSuccess {
                rejected_data_points: 1,
                error_message: "SECRET_SENTINEL".into(),
            }),
        }
        .encode_to_vec(),
        _ => ExportTraceServiceResponse {
            partial_success: Some(ExportTracePartialSuccess {
                rejected_spans: 1,
                error_message: "SECRET_SENTINEL".into(),
            }),
        }
        .encode_to_vec(),
    }
}
