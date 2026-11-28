//! Connection writes must honor application shutdown and the admitted access deadline.

use super::*;
use axum::{Router, extract::WebSocketUpgrade, routing::get};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn stopped_or_expired_connection_does_not_write_a_native_reply() -> anyhow::Result<()> {
    struct Outcomes {
        stopped: bool,
        expired: bool,
    }
    let (completed, mut outcome) = tokio::sync::mpsc::channel(1);
    // The route supplies an actual Axum socket, without a substitute sink or production hook.
    let route = Router::new().route(
        "/",
        get(move |upgrade: WebSocketUpgrade| {
            let completed = completed.clone();
            async move {
                upgrade.on_upgrade(move |mut socket| async move {
                    let cancelled = CancellationToken::new();
                    cancelled.cancel();
                    let message = || ServerMessage::Failure {
                        kind: FailureKind::Invalid,
                        message: "Connection ended".into(),
                    };
                    let stopped = send(
                        &mut socket,
                        &cancelled,
                        jiff::Timestamp::now() + jiff::SignedDuration::from_mins(1),
                        message(),
                    )
                    .await
                    .is_err();
                    let expired = send(
                        &mut socket,
                        &CancellationToken::new(),
                        jiff::Timestamp::now() - jiff::SignedDuration::from_secs(1),
                        message(),
                    )
                    .await
                    .is_err();
                    let _ = completed.send(Outcomes { stopped, expired }).await;
                })
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, route).await });
    let (_peer, _) = tokio_tungstenite::connect_async(format!("ws://{address}/")).await?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), outcome.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("connection result missing"))?;
    server.abort();
    assert!(result.stopped, "shutdown must prevent a new socket write");
    assert!(
        result.expired,
        "expired access must prevent a new socket write"
    );
    Ok(())
}

#[test]
fn failure_classification_excludes_untrusted_error_details() {
    use crate::oauth::error::ClientAuthError;
    let operational = anyhow::anyhow!("PRIVATE-PAYLOAD-CANARY");
    let (kind, message) = failure::classify(&operational);
    assert_eq!(kind, FailureKind::Unavailable);
    assert!(!message.contains("PRIVATE-PAYLOAD-CANARY"));

    let invalid = failure::InvalidMessage {
        reason: "Discovery checkpoint belongs to another account",
    };
    let (kind, message) = failure::classify(&anyhow::Error::new(invalid));
    assert_eq!(kind, FailureKind::Invalid);
    assert_eq!(message, "Discovery checkpoint belongs to another account");

    let (kind, _) = failure::classify(&ClientAuthError::Invalid.into());
    assert_eq!(kind, FailureKind::Unauthorized);
    let (kind, _) = failure::classify(&ClientAuthError::Unavailable.into());
    assert_eq!(kind, FailureKind::Unavailable);
}
